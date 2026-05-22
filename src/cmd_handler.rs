use std::cell::Cell;
use std::collections::{BTreeMap, HashSet, btree_map};
use std::collections::{HashMap, VecDeque, hash_map::Entry };
use std::iter;
use std::ops::Bound::{Included, Excluded, Unbounded};
use std::rc::Rc;
use std::str::from_utf8;
use std::string::ParseError;
use std::sync::atomic::{ AtomicU64, Ordering };

use base64::write;
use libc::{key_t, write};

use base64::{Engine, engine::general_purpose::STANDARD};

use crate::resp::{ RespType, RespValue };
use crate::epoll::{add_interest, get_epoll_event_read, remove_interest, timer_create_event};
use crate::cmd_builder::{ Cmd, CmdArg, CmdError, KW_ACK, KW_FULLRESYNC, KW_GETACK, KW_PONG, KW_QUEUED, KW_REPLCONF, KW_REPLICATION };
use crate::utils::now;
use crate::{ AppStates, ReplStats };
use crate::ClientTable;

// Stored value types for CmdHandler
#[derive(Debug)]
enum StoreValue {
    Str(String),
    List(VecDeque<String>),
    Set(HashSet<String>),
    ZSet(BTreeMap<String, f64>),
    Hash(HashMap<String, String>),
    // timestamp id - (timestamp, order, Vec of String)
    Stream(BTreeMap<(u64, u64), Vec<String>>),
    VectorSet(String),
    None
}

impl StoreValue {
    const fn get_type(&self) -> &str {
        match self {
            Self::Str(_) => "string", 
            Self::List(_) => "list",
            Self::Set(_) => "set",
            Self::ZSet(_) => "zset",
            Self::Hash(_) => "hash",
            Self::Stream(_) => "stream",
            Self::VectorSet(_) => "vectorset",
            _ => "none",
        }
    }
}

#[derive(Debug)]
struct StoreItem { value: StoreValue, expired_at: Option<u64> }

type Task = Box<dyn FnOnce(&mut CmdHandler)>;

enum RequestEntry {
    List {
        client_id: u64, key: String, deadline: u64,
        backlog_task: Task, deadline_task: Task, },
    Stream {
        client_id: u64, key_ts: Vec<(String, u64, u64)>, // key, ts, seq
        deadline: u64, backlog_task: Task, deadline_task: Task,
    },
    Wait {
        client_id: u64, required_count: u64, actual_count: Rc<Cell<u64>>,
        deadline: u64, offset_snapshot: i64,
        slave_ids: Vec<u64>, deadline_task: Task,
    },
}

impl RequestEntry {
    pub fn set_deadline(&mut self, d: u64){
        match self {
            Self::List { deadline, .. } => {
                *deadline = d
            },
            Self::Stream { deadline, .. } => {
                *deadline = d
            },
            Self::Wait {deadline, .. } => {
                *deadline = d
            }
        }
    }
}

struct RequestRegistry {
    // timestamp - request, each request has an unique timestamp id
    store: HashMap<u64, RequestEntry>,
    
    // waiting queue for list
    // list name - queue of client (timestmap id)
    backlog_list: HashMap<String, VecDeque<u64>>,

    // Set of client waiting for stream
    // This is not FIFO, but scan through all waiting clients
    // as XREAD is fanning-out, not consuming
    backlog_stream: HashSet<u64>,

    // Transaction, the Vec<u8> is for broadcasting
    backlog_txn: HashMap<u64, VecDeque<(Cmd, Vec<u8>)>>,
    
    // Watch list, key - (client_id - dirty)
    watchlist: HashMap<String, HashMap<u64, bool>>,

    // To store waiting WAIT for client, client_id - timestamp
    backlog_wait: HashMap<u64, u64>,

    // To store client must send back ACK, slave_id - list of client_id
    // master could receive multiple WAIT from multiple clients
    backlog_ack: HashMap<u64, Vec<u64>>,

    // deadline - timestamp
    deadline: BTreeMap<u64, u64>,
    timer_fd: i32,
}

impl RequestRegistry {
    pub fn new(timer_fd: i32) -> Self {
        Self {
            store: HashMap::new(),
            backlog_list: HashMap::new(),
            backlog_stream: HashSet::new(),
            backlog_txn: HashMap::new(),
            watchlist: HashMap::new(),
            backlog_wait: HashMap::new(),
            backlog_ack: HashMap::new(),
            deadline: BTreeMap::new(),
            timer_fd,
        }
    }
    
    pub fn insert(
        &mut self,
        timestamp: u64,
        deadline: u64,
        mut request_entry: RequestEntry) {
        // Insert a RequestEntry to the Registry storage structure
        // A RequestEntry will be stored in: 
        // - self.store as the center storage, distinguished by signature timestamp
        // - a deadline queue, to signify what action to take when that Entry expires,
        //   the signature is deadline, also unique for each entry
        // - a dedicated list/hashmap/hashset etc. to store the fullfilment progress
        if deadline > 0 {
            let mut d = deadline;
            while self.deadline.contains_key(&d) {
                // This is to avoid deadline collision, 
                // no 2 or more Entry will have the same deadline
                d += 1
            };
            self.deadline.insert(d, timestamp);
            request_entry.set_deadline(d);
            self.set_timer_fd();
        };

        match &request_entry {
            RequestEntry::List { key, .. } => {
                self.backlog_list.entry(key.clone())
                    .or_insert_with(VecDeque::new).push_back(timestamp);
            },
            RequestEntry::Stream { .. } => {
                self.backlog_stream.insert(timestamp);
            },
            RequestEntry::Wait { client_id, slave_ids, .. } => {
                self.backlog_wait.insert(*client_id, timestamp);
                for i in slave_ids {
                    self.backlog_ack.entry(*i).or_insert_with(Vec::new).push(*client_id); 
                }
            }
        }
            
        self.store.insert(timestamp, request_entry);
    }
    
    pub fn remove(&mut self, timestamp: &u64) -> Option<RequestEntry> {
        // Remove a RequestEntry completely
        // This could be call more than once without error
        let entry = self.store.remove(timestamp)?;
        
        let deadline = match entry {
            RequestEntry::List { ref key, deadline, .. } => {
                if let Some(q) = self.backlog_list.get_mut(key) {
                    // This will have no effect if backlog is pooped
                    // when a backlog condition fullfilled
                    q.retain(|&t| t!= *timestamp);
                    if q.is_empty() {
                        self.backlog_list.remove(key);
                    };
                };
                deadline
            },
            RequestEntry::Stream { deadline, .. } => {
                self.backlog_stream.remove(timestamp);
                deadline
            },
            RequestEntry::Wait { client_id, deadline, ref slave_ids, .. } => {
                self.backlog_wait.remove(&client_id);
                for i in slave_ids {
                    self.backlog_ack.entry(*i).and_modify(|v| v.retain(|x| *x != client_id));
                }
                deadline
            }
        };
        
        self.deadline.remove(&deadline);
        self.set_timer_fd();
        Some(entry)
    }

fn set_timer_fd(&self) {
        match self.deadline.first_key_value() {
            Some((deadline, _)) => {
                let now = now();
                let timeout = if *deadline > now {
                    (deadline - now) as i64
                } else {
                    // Schedule to be fired immediately in next loop
                    // unit ms
                    1
                };
                println!("Set timer for deadline {}, timeout {}", deadline, timeout);
                timer_create_event(self.timer_fd, timeout);
            },
            None => {},
        }
    }

    pub fn is_backlog_list_empty(&self, key: &str) -> bool {
        match self.backlog_list.get(key) {
            Some(list) => {
                list.is_empty()
            },
            None => false
        }
    }

    pub fn get_nearest_deadline(&self) -> Option<(&u64, &u64)> {
        self.deadline.first_key_value() 
    }

    pub fn add_to_txn(&mut self, client_id: u64, cmd: Cmd, buf: Vec<u8>) {
        self.backlog_txn.entry(client_id)
            .or_insert_with(VecDeque::new).push_back((cmd, buf));
    }

    pub fn watch(&mut self, mut keys: Vec<String>, client_id: u64) {
        keys.drain(..).for_each(|k| {
            self.watchlist.entry(k).or_insert_with(HashMap::new)
                .entry(client_id).or_insert(false);
        }) 
    }

    pub fn unwatch_all(&mut self, client_id: &u64) {
        for (_, v) in self.watchlist.iter_mut() {
            v.remove(client_id); 
        }
    }

    pub fn set_dirty(&mut self, cmd: &Cmd) {
        match cmd {
            Cmd::SET { key, .. } |
            Cmd::LPUSH { key, .. } | Cmd::RPUSH { key, .. } |
            Cmd::LPOP { key, .. } | Cmd::BLPOP { key, .. } => {
                match self.watchlist.get_mut(key) {
                    Some(w) => {
                        for (client_id, dirty) in w {
                            println!("Set dirty for key {} client_id {}", key, client_id);
                            *dirty = true
                        }
                    },
                    None => {
                        // Not watching any key
                    }
                }
            },
            _ => {/* Not modifying key */}
        }
    }

    pub fn is_any_dirty(&self, client_id: &u64) -> bool {
        self.watchlist.values().any(|clients| {
            clients.get(client_id).copied().unwrap_or(false)
        })
    }
}

//---------Command handler, convert Cmd struct into action
enum CmdOutput {
    Resp(RespType),
    Bytes(Vec<u8>),
    None,
}

impl CmdOutput{
    pub fn extract_resp(self) -> Option<RespType> {
        match self {
            Self::Resp(r) => Some(r),
            _ => None
        }
    }

    pub fn extract_bytes(self) -> Option<Vec<u8>> {
        match self {
            Self::Bytes(v) => Some(v),
            _ => None
        }
    }
}

pub struct CmdHandler {
    // client_id, message
    pub response_queue: Vec<(u64, Vec<u8>)>,
    data: HashMap<String, StoreItem>,
    registry: RequestRegistry,
 }

impl CmdHandler {
    pub fn new(timer_fd: i32) -> Self{
        Self {
            response_queue: Vec::new(),
            data: HashMap::new(),
            registry: RequestRegistry::new(timer_fd),
        }
    }

    fn _execute_cmd(
        &mut self, cmd: Cmd, buf: Vec<u8>, client_id: u64, epoll_fd: Option<i32>,
        serialized: bool) -> Result<CmdOutput, CmdOutput> {
        // Set dirty first
        self.registry.set_dirty(&cmd); 
        let to_be_broadcast = cmd.to_be_broadcast();
        let always_response = cmd.always_response();
        
        // Whether the client is master
        let is_master = ClientTable::get().borrow().is_master(&client_id);
        
        // Add bytes count to offset
        AppStates::get().host_stats.as_ref().unwrap().add_bytes_count(buf.len() as i64);

        println!("Cmd received {:?}, to be broadcast {}", &cmd, &to_be_broadcast);
        let output: Result<Option<RespType>, RespType> = match cmd {
            Cmd::PING => Self::cmd_ping(),
            Cmd::PONG => Ok(None), // handle at client level when handshaking
            Cmd::OK => Ok(None),
            Cmd::ECHO(s) => Self::cmd_echo(s),
            Cmd::SET{ key, value, opt } => self.cmd_set(key, value, opt),
            Cmd::GET{ key } => self.cmd_get(key),
            Cmd::RPUSH{ key, value } => self.cmd_rpush(key, value),
            Cmd::LRANGE { key, start, stop } => self.cmd_lrange(key, start, stop),
            Cmd::LPUSH{ key, value } => self.cmd_lpush(key, value),
            Cmd::LLEN(s) => self.cmd_llen(s),
            Cmd::LPOP{ key, length } => self.cmd_lpop(key, length),
            Cmd::BLPOP { key, timeout_ms } => self.cmd_blpop(key, timeout_ms, client_id),
            Cmd::TYPE(key) => self.cmd_type(key),
            Cmd::XADD { key, id, value } => self.cmd_xadd(key, id, value),
            Cmd::XRANGE { key, start, end } => self.cmd_xrange(key, start, end),
            Cmd::XREAD { count, block_ms, stream } => 
                self.cmd_xread(count, block_ms, stream, client_id),
            Cmd::INCR(key) => self.cmd_incr(key),
            Cmd::MULTI => self.cmd_multi(client_id),
            Cmd::EXEC => self.cmd_exec(client_id),
            Cmd::DISCARD => self.cmd_discard(client_id),
            Cmd::WATCH(keys) => self.cmd_watch(keys, client_id),
            Cmd::UNWATCH => self.cmd_unwatch(client_id),
            Cmd::INFO(key) => Self::cmd_info(key),
            Cmd::PSYNC { id, offset } => self.cmd_psync(id, offset, client_id),
            Cmd::REPLCONF(arg) => self.cmd_replconf(arg, client_id, epoll_fd),
            Cmd::FULLRESYNC { .. } => Ok(None),
            Cmd::RDB(_) => Ok(None), // client level
            Cmd::WAIT { count, timeout_ms } => self.cmd_wait(count, timeout_ms, client_id, epoll_fd),
            // _ => None
        };

        println!("Cmd response {:?}", &output);
        match output {
            Ok(Some(resp)) => {
                println!("Success cmd from client {}, broadcasting {}", &client_id, &to_be_broadcast);
                if to_be_broadcast {
                    // Broadcast by push to response_queue
                    match ClientTable::get().borrow().list_slave() {
                        Some(list) => {
                            for id in list {
                                self.response_queue.push((*id, buf.clone()));
                            }
                        }
                        None => {} 
                    }
                }
                // }
                
                // If sending client is master, not response 
                // unless it's REPLCONF GETACK
                if !is_master | always_response {
                    if serialized {
                        Ok(CmdOutput::Bytes(resp.to_bytes().unwrap()))
                    } else {
                        Ok(CmdOutput::Resp(resp))
                    }
                } else {
                    Ok(CmdOutput::None)
                }
                
                // Command propagation should be done here
                // so work in both case of normal execution and txn execution
                
            },
            Err(resp) => {
                if serialized {
                    Err(CmdOutput::Bytes(resp.to_bytes().unwrap()))
                } else {
                    Err(CmdOutput::Resp(resp))
                }
            },
            _ => Ok(CmdOutput::None)
        }
    }

    pub fn handle(
        &mut self, cmd: Result<Cmd, CmdError>, buf: Vec<u8>, client_id: u64, epoll_fd: i32)
        -> Result<Option<Vec<u8>>, Vec<u8>> {
        match cmd {
            Ok(c) => {
                if !self.registry.backlog_txn.contains_key(&client_id) {
                    self._execute_cmd(c, buf, client_id, Some(epoll_fd), true)
                        .map(|out| {out.extract_bytes()})
                        .map_err(|err| err.extract_bytes().unwrap_or_default())
                } else {
                    match c {
                        Cmd::EXEC => {
                            // Execute transaction
                            match self.cmd_exec(client_id) {
                                Ok(Some(resp)) => Ok(resp.to_bytes()),
                                _ => Ok(None),
                            }
                        },
                        Cmd::DISCARD => {
                            match self.cmd_discard(client_id) {
                                Ok(Some(resp)) => Ok(resp.to_bytes()),
                                _ => Ok(None),
                            }
                        },
                        Cmd::WATCH(_) => {
                            Ok(RespType::Error(Some(
                                "ERR WATCH inside MULTI is not allowed".to_string()))
                                .to_bytes())
                        }
                        _ => {
                            // Building a transaction
                            self.registry.add_to_txn(client_id, c, buf);
                            Ok(RespType::SimpleStr(Some(KW_QUEUED.to_string())).to_bytes())
                        }
                    }
                }
                
            },
            Err(e) => {
                if let Ok(Some(e)) = Self::cmd_err(e.to_string()) {
                    Err(e.to_bytes().unwrap())
                } else {
                    Err(Vec::from("Unexpected error"))
                }
            }
        } 
    }

    fn extract_deadline(opt: Option<CmdArg>) -> Option<u64> {
        match opt {
            Some(arg) => {
                let now = now(); 
                match arg {
                    CmdArg::EX(x) => Some(now / 1000 + x.unwrap()),
                    CmdArg::PX(x) => Some(now + x.unwrap()),
                    _ => None,
                }
            },
            None => None
        }
    }
    
    fn get_deadline(timeout_ms: Option<u64>) -> (u64, u64) {
        let now = now();
        match timeout_ms{
            Some(timeout) => {
                (now, now + timeout as u64)
            },
            None => (now, 0 as u64)
        }
    }

    fn get_null_array() -> Option<Vec<u8>> {
        RespType::Array { length: 0, value: None }.to_bytes()
    }

    pub fn callback_deadline_expire(&mut self) {
        // A callback fired when deadline expired (triggere by timer fd):
        // Execute callback in deadline_task
        let timestamp = *self.registry.get_nearest_deadline().unwrap().1;
        match self.registry.remove(&timestamp) {
            Some(entry) => match entry {
                RequestEntry::List { deadline_task, .. } => {
                    deadline_task(self);
                },
                RequestEntry::Stream { deadline_task, .. } => {
                    deadline_task(self);
                },
                RequestEntry::Wait { deadline_task, .. } => {
                    deadline_task(self)
                }
            },
            None => { },
        };
    }

    pub fn response_ok() -> Option<RespType> {
        Some(RespType::SimpleStr(Some("OK".to_string())))
    }
    
    pub fn serve_backlog_list(&mut self) {
        // Execute once at the end of main each event loop,
        // matching BLPOP queue with available item,
        // other LPOP requests are served at request time at client loop
        let mut timestamps: Vec<u64> = Vec::new();

        for (k, v) in self.registry.backlog_list.iter() {
            if let Some(item) = self.data.get(k) {
                match &item.value {
                    StoreValue::List(list) => {
                        let max_pop = v.len().min(list.len());
                        v.iter().take(max_pop).for_each(|x| timestamps.push(*x));
                    },
                    _ => {},
                }
            }
        };

        let mut tasks: Vec<Task> = Vec::new();
        for t in &timestamps {
            if let Some(entry) = self.registry.remove(t) {
                // Task may be fullfilled
                match entry {
                    RequestEntry::List { backlog_task, .. } => {
                        tasks.push(backlog_task);
                    },
                    _ => {}
                }
            }
        };
        
        // Execute backlog task
        for task in tasks {
            task(self);
        }
    }

    pub fn serve_backlog_stream(&mut self) {
        // Consecutively scanned for fullfilled stream
        // in backlog_stream
        let mut timestamps: Vec<u64> = Vec::new();
        for t in self.registry.backlog_stream.iter() {
            println!("Timestamp {} in backlog_stream", &t);
            let mut has_items = false;
            match self.registry.store.get(&t) {
                Some(entry) => match entry {
                    // Checking for streams having item
                    RequestEntry::Stream { key_ts, .. }   => {
                        for (key, ts, seq) in key_ts {
                            if self._is_stream_avail(key, *ts, *seq) {
                                has_items = true;
                                break
                            } 
                        }
                    },
                    _ => {},
                },
                None => {}
            };
            
            if has_items { 
                println!("Having item for timestamp {}", t);
                timestamps.push(*t) };
        };
        
        let mut tasks: Vec<Task> = Vec::new();
        for t in &timestamps {
            if let Some(entry) = self.registry.remove(t) {
                // Task may be fullfilled
                match entry {
                    RequestEntry::Stream { backlog_task, .. } => {
                        tasks.push(backlog_task);
                        println!("Pop out backlog task for timestamp {}", t);
                    },
                    _ => {}
                }
            }
        };
        
        // Execute backlog task
        for task in tasks {
            task(self);
        }
    }

    fn cmd_ping() -> Result<Option<RespType>, RespType> {
        Ok(Some(RespType::SimpleStr(Some(KW_PONG.to_string()))))
    }

    fn cmd_echo(s: String) -> Result<Option<RespType>, RespType> {
        Ok(Some(RespType::BulkStr{ length: s.len(), value: Some(s) }))
    }

    fn cmd_err(s: String) -> Result<Option<RespType>, RespType> {
        Ok(Some(RespType::SimpleStr(Some(s))))
    }

    fn cmd_set(&mut self, key: String, value: String, opt: Option<CmdArg>) ->
        Result<Option<RespType>, RespType> {
        // Extract cmd for handling instruction
        let exp = Self::extract_deadline(opt); 
        self.data.insert(key, StoreItem {
            value: StoreValue::Str(value), expired_at: exp});
        Ok(Self::response_ok())
    }

    fn cmd_get(&self, key: String) -> Result<Option<RespType>, RespType> {
        match self.data.get(&key) {
            Some(item) => match &item.value {
                StoreValue::Str(s) => {
                    // Check for expiration
                    // TODO:: implement removed at expiration
                    let expired = item.expired_at.map_or(false, |x| now() > x);
                    let value = if expired { None } else { Some(s.clone()) };
                    Ok(Some(RespType::BulkStr{
                        length: value.as_ref().map_or(0, |s| s.len()),
                        value: value
                    }))
                },
                _ => Err(RespType::Error(
                    Some(CmdError::UnsupportedCommand(key).to_string()))
                    )
            },
            // No key found
            None => Ok(Some(RespType::BulkStr{ length: 0, value: None })),
        } 
    }

    fn cmd_rpush(&mut self, key: String, value: Vec<String>) ->
        Result<Option<RespType>, RespType> {
        let item = self.data.entry(key.clone()).or_insert(
            StoreItem {
                value: StoreValue::List(VecDeque::new()),
                expired_at: None });
        
        match &mut item.value {
            StoreValue::List(list) => {
                list.extend(value);
                Ok(Some(RespType::Integer(Some(list.len() as i64))))
            },
            _ =>  Err(RespType::Error(
                Some(CmdError::UnsupportedCommand(key).to_string()))
            )
        }
    }

    fn cmd_lrange(&self, key: String, start: i64, stop: i64) -> Result<Option<RespType>, RespType> {
        // Check valid key First
        match self.data.get(&key) {
            Some(item) => {
                match &item.value {
                    StoreValue::List(list) => {
                        // Edge case
                        if start > stop && start > 0 && stop > 0 {
                            return Ok(Some(RespType::Array{ length: 0, value: Some(VecDeque::new())}))
                        };
                        
                        // VecDeque could wrap it self, so need this
                        if start >= 0 && start as usize > list.len() - 1 {
                            return Ok(Some(RespType::Array{ length: 0, value: Some(VecDeque::new())}))
                        };

                        // Convert negative index to positive index
                        let start_abs: i64;
                        let stop_abs: i64;

                        if start < 0 {
                            start_abs = (list.len() as i64 + start).max(0);
                        } else {
                            start_abs = start;
                        }

                        if stop < 0 {
                            stop_abs = (list.len() as i64 + stop).max(0);
                        } else {
                            stop_abs = stop;
                        };

                        let max_index = stop_abs.min(list.len() as i64 - 1) as usize;
                        let min_index = start_abs.min(list.len() as i64 - 1) as usize;
                        let output_len = max_index - min_index + 1;
                        if output_len == 0 {
                            Ok(Some(RespType::Array{
                                length: output_len, 
                                value: Some(VecDeque::new())
                            }))
                        } else {
                            let mut output = RespType::Array{
                                length: output_len as usize,
                                value: Some(VecDeque::<RespType>::new())};
                            
                            for i in list.iter().skip(min_index).take(output_len){
                                output.add_item(RespType::BulkStr { length: i.len(), value: Some(i.clone()) }); 
                            };
                            Ok(Some(output))
                        }
                    },
                    _ => Err(RespType::Error(
                        Some(CmdError::UnsupportedCommand(key).to_string())
                    ))
                }
            },
            None => {
                Ok(Some(RespType::Array{ length: 0, value: Some(VecDeque::new()) }))
            }
        }
    }
    
    fn cmd_lpush(&mut self, key: String, value: VecDeque<String>) -> Result<Option<RespType>, RespType> {
        // Create list if not exists
        let item = self.data.entry(key.clone()).or_insert(
            StoreItem { 
                value: StoreValue::List(VecDeque::new()),
                expired_at: None } );

        match &mut item.value {
            StoreValue::List(list) => {
                for v in value {
                    list.push_front(v)
                };
                Ok(Some(RespType::Integer(Some(list.len() as i64))))
            },
            _ => Err(RespType::Error(
                Some(CmdError::UnsupportedCommand(key).to_string())
            ))
        }
        
    }

    fn cmd_llen(&self, key: String) -> Result<Option<RespType>, RespType> {
        match self.data.get(&key) {
            Some(item) => {
                match &item.value {
                    StoreValue::List(list) => {
                        Ok(Some(RespType::Integer(Some(list.len() as i64))))
                    },
                    _ => {
                        Err(RespType::Error(
                            Some(CmdError::UnsupportedCommand(key).to_string())
                        ))
                    }
                }
            },
            None => Ok(Some(RespType::Integer(Some(0))))
        }
    }
    
    fn cmd_lpop(&mut self, key: String, length: Option<usize>) -> Result<Option<RespType>, RespType> {
        match self.data.get_mut(&key) {
            Some(item) => {
                match &mut item.value {
                    StoreValue::List(list) => {
                        match length {
                            Some(x) => {
                                let take_nbr = x.min(list.len());
                                let mut output = RespType::Array { length: take_nbr, value: None };
                                for v in list.drain(..take_nbr) {
                                    output.add_item(
                                        RespType::BulkStr { length: v.len(), value: Some(v) }); 
                                };
                                Ok(Some(output))
                            },
                            None => {
                                match list.pop_front() {
                                    Some(s) => {
                                        Ok(Some(RespType::BulkStr {
                                            length: s.len(), value: Some(s)}))
                                    },
                                    None => {
                                        Ok(Some(RespType::BulkStr {
                                            length: 0, value: None }))
                                    }
                                }
                            }
                        }
                    },
                    _ => {
                        Err(RespType::Error(
                            Some(CmdError::UnsupportedCommand(key).to_string())
                        ))
                    }
                }
            },
            None => {
                Ok(Some(RespType::BulkStr { length: 0, value: None }))
            }
        }         
    }

    fn cmd_blpop(
        &mut self, 
        key: String, 
        timeout_ms: Option<u64>, 
        client_id: u64) -> Result<Option<RespType>, RespType>
    {
        let (timestamp, deadline) = Self::get_deadline(timeout_ms);
        let key_task = key.clone();
        println!("BLPOP at {}, key {}, timeout ms {:?}", deadline, &key, timeout_ms);

        // Task to run when item available to pop
        // Checking for key and type must be done by parent calls this task
        let backlog_task = Box::new(move |handler: &mut CmdHandler| {
            // Task triggered when item available
            println!("Running backlog task for BLPOP at {}", deadline);
            
            let popped = handler.data.get_mut(&key_task)
                .and_then(|item| match &mut item.value {
                    StoreValue::List(list) => {
                        list.pop_front()
                    },
                    _ => None,
                });
            println!("Item available {:?}", popped);

            if let Some(item) = popped {
                // Construct response
                let mut output = RespType::Array { length: 2, value: None };
                let resp_key = RespType::BulkStr { length: key_task.len(), value: Some(key_task) };
                let resp_item = RespType::BulkStr { length: item.len(), value: Some(item) };
                output.add_item(resp_key);
                output.add_item(resp_item);
                handler.response_queue.push((client_id, output.to_bytes().unwrap()));
                println!("Push to response queue: , {:?}", &handler.response_queue.last().unwrap());

                // Disable deadline
                handler.registry.remove(&timestamp);
            };
        });

        let deadline_task = Box::new(move |handler: &mut CmdHandler| {
            let msg = Self::get_null_array().unwrap();
            handler.response_queue.push((client_id, msg));
            println!(
                "Push to response queue: {:?}", 
                &handler.response_queue.last().unwrap());
            handler.registry.remove(&timestamp);
        });
 
        match self.data.get_mut(&key) {
            // Key exists
            Some(item) => {
                match &mut item.value {
                    StoreValue::List(list) => {
                        // If key-list exists, key-backlog queue must exists
                        // as being created by rpush and lpush
                        // let backlog = self.backlog.get_mut(&key1).unwrap();
                        // if backlog.is_empty() && !list.is_empty() {
                        if self.registry.is_backlog_list_empty(&key) && !list.is_empty() {
                            // No other client in waiting list and item available, pop
                            let item = list.pop_front().unwrap();
                            let mut output = RespType::Array{ length: 2, value: None};
                            let list_name = RespType::BulkStr { length: key.len(), value: Some(key) };
                            let value = RespType::BulkStr { length: item.len(), value: Some(item) };
                            output.add_item(list_name);
                            output.add_item(value);
                            Ok(Some(output))
                        } else {
                            // Wait
                            self.registry.insert(
                                timestamp, deadline, 
                                RequestEntry::List { client_id, key, deadline, backlog_task, deadline_task });
                            Ok(None)
                        }
                    },
                    _ => Err(RespType::Error(
                        Some(CmdError::UnsupportedCommand(key).to_string())
                    ))
                }
            },
            None => {
                // No key-list exists, also no key-backlog exists
                // wait in queue for client
                self.registry.insert(
                    timestamp, deadline,
                    RequestEntry::List { client_id, key, deadline, backlog_task, deadline_task });
                Ok(None)
            }
        }
    }

    fn cmd_type(&self, key: String) -> Result<Option<RespType>, RespType> {
        let ktype = match self.data.get(&key) {
            Some(item) => {
                item.value.get_type().to_string()
            },
            None => "none".to_string()
        };
        Ok(Some(RespType::SimpleStr(Some(ktype))))
    }

    fn cmd_xadd(
        &mut self,
        key: String,
        id: (Option<u64>, Option<u64>), 
        // For this stage, id is explicitely declare
        // TODO: handle case id = * and id = 123-*
        value: Vec<String>) -> Result<Option<RespType>, RespType> {
        // Add key if not exists
        self.data.entry(key.clone()).or_insert(StoreItem {
            value: StoreValue::Stream(BTreeMap::new()),
            expired_at: None });

        // Error messages
        let msg_invalid_last = RespType::Error(Some(
           "ERR The ID specified in XADD is equal \
           or smaller than the target stream top item".to_string())
        );

        let msg_invalid_00 = RespType::Error(Some(
            "ERR The ID specified in XADD must be greater than 0-0".to_string())
        );

        let msg_invalid_format = RespType::Error(Some(
            "ERR Invalid stream id format".to_string())
        );

        // Check valid type
        let id_valid: (u64, u64) = match &self.data.get(&key).unwrap().value {
            StoreValue::Stream(b) => {
                match id {
                    (Some(t), Some(i)) => {
                        if t == 0 && i == 0 {
                            return Err(msg_invalid_00)
                        };

                        match b.last_key_value() {
                            Some((k, _)) => {
                                let (t_last, i_last) = *k;
                                if t > t_last || (t == t_last && i > i_last) {
                                    (t, i)
                                } else {
                                    return Err(msg_invalid_last)
                                } 
                            },
                            None => (t, i),
                        }
                    },
                    (Some(t), None) => {
                        match b.last_key_value() {
                            Some((k, _)) => {
                                let (t_last, i_last) = *k;
                                if t == t_last {
                                    (t, i_last + 1)
                                }
                                else if t > t_last {
                                    (t, 0)
                                } else {
                                    return Err(msg_invalid_last)
                                }
                            },
                            None => {
                                if t == 0 {(t, 1)} else {(t, 0)}
                            }
                        } 
                    },
                    (None, None) => {
                        (now(), 0)
                    },
                    _ => {
                        return Err(msg_invalid_format)
                    }
                }
            },
            _ => {
                return Err(RespType::Error(Some(
                    CmdError::UnsupportedCommand(key).to_string())
                ));
            }

        };

        match &mut self.data.get_mut(&key).unwrap().value {
            StoreValue::Stream(btree) => {
                btree.insert((id_valid.0, id_valid.1), value);
                let msg = format!("{}-{}", id_valid.0, id_valid.1);
                Ok(Some(RespType::BulkStr { length: msg.len(), value: Some(msg) }))
            },
            _ => Ok(None)
        }
    }
    
    fn _extract_stream_id(
        id: (Option<u64>, Option<u64>),
        end: bool) -> Option<(u64, u64)> {
        match id {
            (Some(i), Some(j)) => Some((i, j)),
            (Some(i), None) => {
                if !end {
                    Some((i, 0))
                } else {
                    Some((i, u64::MAX))
                }
            },
            _ => None
        }
    }

    fn _get_resp_stream_entity(kv: &Vec<((u64, u64), Vec<String>)>) -> Vec<RespType> {
        // Construct RespType for an stream entity
        // E.g.:: 0-1 apple banana
        let mut output: Vec<RespType> = Vec::new(); 

        for (k, v) in kv {
            let key: String = format!("{}-{}", k.0, k.1);
            let k_type = RespType::BulkStr {
                length: key.len(),
                value: Some(key)
            };

            let mut v_type = RespType::Array {
                length: v.len(), value: Some(VecDeque::new())
            };

            v.iter().for_each(|s| {
                v_type.add_item(RespType::BulkStr {
                    length: s.len(), value: Some(s.to_string()) });
            }); 
            
            let mut kv_arr = RespType::Array { length: 2, value: Some(VecDeque::new()) };
            kv_arr.add_item(k_type);
            kv_arr.add_item(v_type);
            output.push(kv_arr);
        };
        output
    }

    fn cmd_xrange(
        &mut self,
        key: String,
        start: (u64, u64),
        end: (u64, u64)) -> Result<Option<RespType>, RespType> {
        match self.data.get(&key) {
            Some(item) => {
                match &item.value  {
                    StoreValue::Stream(b) => {
                        let mut kv_vec = Vec::new();
                        for (k, v) in b.range(start..=end) {
                            kv_vec.push((k.clone(), v.clone()));
                        };
                        
                        // Prepare response
                        let mut output = RespType::Array {
                            length: kv_vec.len(), 
                            value: Some(VecDeque::new())
                        };
                        
                        let mut kv_arr = Self::_get_resp_stream_entity(&kv_vec);
                        kv_arr.drain(..).for_each(|t| output.add_item(t));

                        Ok(Some(output))
                    },
                    _ => return Err(RespType::Error(Some(
                            CmdError::UnsupportedCommand(key).to_string())
                        )),
                }
            },
            None => return Ok(Some(RespType::Array {
                    length: 0,
                    value: Some(VecDeque::new())
                })),
        }
    }

    fn _extract_stream_entries(
        &self, 
        key: &str,
        count: u64, timestamp: u64, seq: u64) -> Option<Box<RespType>>{
        /*
        Extract from a stream btree given key name

        Params:
        - count: nbr of entities needed to be read
        - timestamp: timestamp part of stream id to read onwards
        - seq: sequence part of stream id to read onwards
        
        Return:
        - RespType Array, 3 nested level key - id - entities
        */ 

        let mut entries = match self.data.get(key) {
            Some(item) => match &item.value {
                StoreValue::Stream(b) => {
                    b.range((Excluded((timestamp, seq)), Unbounded)) 
                        .take(count as usize)
                        .map(|(k, v)| (*k, v.clone()))
                        .collect()
                },
                _ => { 
                    Vec::new()
                },
            },
            None => {
                return None 
            } 
        };
        
        if entries.is_empty() { return None };
        
        // Construct return RespType
        // Blank array, not null array
        let mut arr_entries = RespType::Array {
            length: entries.len(),
            value: Some(VecDeque::new()) };
        for ((ts, sq), mut v) in entries.drain(..) {
            let mut arr_entry = RespType::Array { length: 2, value: None };
            let msg = format!("{ts}-{sq}");
            let stream_id = RespType::BulkStr { length: msg.len(), value: Some(msg) }; 
            let mut stream_content = RespType::Array { length: v.len(), value: None };
            v.drain(..).for_each(|s| {
                stream_content.add_item(
                    RespType::BulkStr { length: s.len(), value: Some(s) }
                );
            });

            arr_entry.add_item(stream_id);
            arr_entry.add_item(stream_content);
            arr_entries.add_item(arr_entry);
        };
        Some(Box::new(arr_entries))
    }

    fn _is_stream_avail(&self, key: &str, start_ts: u64, seq: u64) -> bool {
        // To check for available item in stream key
        match self.data.get(key) {
            Some(item) => match &item.value {
                StoreValue::Stream(b) => {
                    b.range((Excluded((start_ts, seq)), Unbounded)) 
                        .next().is_some()
                },
                _ => { 
                    false 
                },
            },
            None => {
                false 
            } 
        }
    }

    fn cmd_xread(
        &mut self,
        count: Option<u64>,
        block: Option<u64>,
        stream: Vec<(String, u64, u64)>,
        client_id: u64) -> Result<Option<RespType>, RespType> {
        // Check valid type first
        for (key, _, _) in &stream{
            match self.data.get(key) {
                Some(item) => match &item.value {
                    StoreValue::Stream(_) => {/* Do nothing */},
                    _ => return Err(RespType::Error(Some(
                            CmdError::UnsupportedCommand(key.clone()).to_string())
                        ))
                },
                None => { /* Do nothing */ }
            }
        };

        // Resolve $ sentinel (u64::MAX, u64::MAX) to the current last entry ID per stream
        let stream: Vec<(String, u64, u64)> = stream.into_iter().map(|(key, ts, seq)| {
            if ts == u64::MAX && seq == u64::MAX {
                let (rts, rseq) = self.data.get(&key)
                    .and_then(|item| match &item.value {
                        StoreValue::Stream(b) => b.keys().next_back().copied(),
                        _ => None,
                    })
                    .unwrap_or((0, 0));
                (key, rts, rseq)
            } else {
                (key, ts, seq)
            }
        }).collect();

        // Immediate serving if possible
        // Extract resp entries from stream
        // key, resp
        let mut stream_vec: Vec<(String, RespType)> = Vec::new();

        // key, start timestamp, seq
        let mut key_ts: Vec<(String, u64, u64)> = Vec::new();
        for (key, start_ts, seq) in &stream {
            match self._extract_stream_entries(&key, count.unwrap_or(u64::MAX), *start_ts, *seq) {
                Some(b) => { 
                    stream_vec.push((key.clone(), *b)); 
                },
                None => {/* Skip this, no return */}
            };
            key_ts.push((key.clone(), *start_ts, *seq));
        };
        
        // If non blocking, return nil if no entities retrieved
        if stream_vec.is_empty() {
            match block {
                Some(t_ms) => {
                    // Put to wait queue 
                    // BLOCK 0 means wait indefinitely — pass None so no deadline timer is armed
                    let timeout_arg = if t_ms == 0 { None } else { Some(t_ms as u64) };
                    let (timestamp, deadline) = Self::get_deadline(timeout_arg);
                    
                    // This will run if conditions meet
                    let mut keys: Vec<String> = Vec::new();
                    for (key, _, _) in &stream {
                        keys.push(key.clone());
                    }

                    let backlog_task = Box::new(move |handler: &mut CmdHandler| {
                        let mut stream_vec: Vec<(String, RespType)> = Vec::new();
                        for (key, start_ts, seq) in stream {
                            println!("Checking stream name {}", &key);
                            match handler._extract_stream_entries(
                                &key, count.unwrap_or(u64::MAX), start_ts, seq
                                ) {
                                Some(b) => {
                                    stream_vec.push((key, *b));
                                },
                                None => {/* Skip this, no return */
                                    println!("No available item for stream {}", &key);
                                }
                                };
                        };

                        if stream_vec.is_empty() {
                           handler.response_queue.push((
                                   client_id,
                                   RespType::Array { length: 0, value: None }.to_bytes().unwrap()));
                        } else {
                            let mut arr_stream = RespType::Array {
                                length: stream_vec.len(), value: None };
                            for (key, entries) in stream_vec.drain(..) {
                                let mut pair = RespType::Array { length: 2, value: None };
                                pair.add_item(RespType::BulkStr { length: key.len(), value: Some(key) });
                                pair.add_item(entries);
                                arr_stream.add_item(pair);
                            };
                            handler.response_queue.push((client_id, arr_stream.to_bytes().unwrap()));
                            handler.registry.remove(&timestamp);
                        }
                    });

                    let deadline_task = Box::new(move |handler: &mut CmdHandler| {
                        let msg = Self::get_null_array().unwrap();
                        handler.response_queue.push((client_id, msg));
                        println!(
                            "Push to response queue: {:?}", 
                            &handler.response_queue.last().unwrap());
                        handler.registry.remove(&timestamp);
                    });

                    self.registry.insert(
                        timestamp, deadline,
                        RequestEntry::Stream { client_id, key_ts, deadline, backlog_task, deadline_task }
                    );
                },   
                None => {
                    // Return blank array
                    return Ok(Some(RespType::Array {
                        length: 0,
                        value: Some(VecDeque::new())
                    }))
                }
            }
        } else {
            // Construct resp response: *N [*2 [key, entries], ...]
            let mut arr_stream = RespType::Array {
                length: stream_vec.len(), value: None };
            for (key, entries) in stream_vec.drain(..) {
                let mut pair = RespType::Array { length: 2, value: None };
                pair.add_item(RespType::BulkStr { length: key.len(), value: Some(key) });
                pair.add_item(entries);
                arr_stream.add_item(pair);
            };
            return Ok(Some(arr_stream))
        }

        // Put to backlog if not fullfilled
        Ok(None)
    }

    fn cmd_incr(&mut self, key: String) -> Result<Option<RespType>, RespType> { 
        let item = self.data.entry(key.clone()).or_insert(
            StoreItem {
                value: StoreValue::Str("0".to_string()),
                expired_at: None,
            }
        );

        let err_out_of_range = RespType::Error(
            Some("ERR value is not an integer or out of range".to_string()) 
        );

        match &mut item.value {
            StoreValue::Str(s) => {
                // Try to convert to i64 first 
                match s.parse::<i64>() {
                    Ok(i) => {
                        if i < i64::MAX {
                            *s = i64::to_string(&(i + 1));
                            Ok(Some(RespType::Integer(Some(i + 1))))
                        } else {
                            Err(
                                RespType::Error(
                                    Some(CmdError::UnprocessableError(
                                        "Intever overflow".to_string()
                                    ).to_string())
                                ))
                        }
                    },
                    Err(_) => Err(err_out_of_range)
                }
            },
            _ => {
                Err(err_out_of_range)
            }
        }
    }
    
    fn cmd_multi(&mut self, client_id: u64) -> Result<Option<RespType>, RespType> {
        let no_txn: bool = !self.registry.backlog_txn.contains_key(&client_id);

        if no_txn {
            self.registry.backlog_txn.insert(client_id, VecDeque::new());
            println!("Open txn for client_id {}", &client_id);
            Ok(Self::response_ok())
        } else {
            Err(RespType::Error(
                Some("ERR MULTI calls can not be nested".to_string()))
                ) 
        }
    }

    fn cmd_exec(&mut self, client_id: u64) -> Result<Option<RespType>, RespType> {
        let is_txn: bool = match self.registry.backlog_txn.get(&client_id) {
            Some(_) => { true },
            None => { false }
        }; 

        if is_txn {
            // Check for dirty first
            println!("Checking dirty for client_id {}", &client_id);
            let has_dirty = self.registry.is_any_dirty(&client_id);

            println!("has_dirty {}", &has_dirty);
            
            if has_dirty {
                self.registry.unwatch_all(&client_id);
                let _ = self.registry.backlog_txn.remove(&client_id);
                return Ok(Some(RespType::Array { length: 0, value: None }));
            };

            let cmd_queue = self.registry.backlog_txn.get_mut(&client_id).unwrap();
            let mut vec_cmd: Vec<(Cmd, Vec<u8>)> = Vec::new();
            let mut vec_resp: Vec<RespType> = Vec::new();
            cmd_queue.drain(..).for_each(|(cmd, buf)| vec_cmd.push((cmd, buf)));
            
            // Execute
            vec_cmd.drain(..).for_each(|(cmd, buf)| {
                    // Cmd will certainly be executed, could unwatch now
                // self.registry.unwatch(&client_id, &cmd);

                match self._execute_cmd(cmd, buf, client_id, None, false) {
                    Ok(cmd_output) | Err(cmd_output) => vec_resp.push(cmd_output.extract_resp().unwrap()), // also need propagation
                }
            });
            
            self.registry.unwatch_all(&client_id);
            let _ = self.registry.backlog_txn.remove(&client_id);
            if !vec_resp.is_empty() {
                let mut arr_output = RespType::Array { length: vec_resp.len(), value: None };
                vec_resp.drain(..).for_each(|r| arr_output.add_item(r));
                Ok(Some(arr_output))
            } else {
                Ok(Some(RespType::Array { length: 0, value: Some(VecDeque::new()) }))
            }
        } else {
            // No transaction opened
            Err(RespType::Error(Some("ERR EXEC without MULTI".to_string()))) 
        }
    }

    fn cmd_discard(&mut self, client_id: u64) -> Result<Option<RespType>, RespType> {
        let is_txn: bool = match self.registry.backlog_txn.get(&client_id) {
            Some(_) => { true },
            None => { false }
        };

        if is_txn {
            let txn = self.registry.backlog_txn.get_mut(&client_id).unwrap();
            txn.drain(..);
            self.registry.backlog_txn.remove(&client_id);
            self.registry.unwatch_all(&client_id);
            Ok(Self::response_ok())
        } else {
            // Error must report st
            Err(RespType::Error(Some("ERR DISCARD without MULTI".to_string())))
        }
    }

    fn cmd_watch(&mut self, keys: Vec<String>, client_id: u64) -> Result<Option<RespType>, RespType> {
        self.registry.watch(keys, client_id);
        Ok(Self::response_ok())
    }

    fn cmd_unwatch(&mut self, client_id: u64) -> Result<Option<RespType>, RespType> {
        self.registry.unwatch_all(&client_id);
        Ok(Self::response_ok())
    }

    fn cmd_info(key: String) -> Result<Option<RespType>, RespType> {
        if key.to_uppercase() == KW_REPLICATION {
            // The slave status initiated with --relicaof
            if AppStates::get().is_slave() {
                // If the sending client master
                let role = "slave"; 
                let stats = AppStates::get().master_stats.as_ref().unwrap();
                let master_host = stats.host.as_ref().unwrap();
                let master_port = stats.port.unwrap();
                let msg = format!(
                    "role:{}\r\nmaster_host:{}\r\nmaster_post:{}",
                    role, master_host, &master_port);
                Ok(Some(RespType::BulkStr { length: msg.len(), value: Some(msg) }))
            } else {
                let role = "master"; 
                let stats = AppStates::get().host_stats.as_ref().unwrap();
                // let master_replid = stats.id.read().unwrap();
                let master_replid = "xxx"; // TODO for testing only
                let master_repl_offset = stats.offset.load().max(0);
                let msg = format!(
                    "role:{}\r\nmaster_replid:{}\r\nmaster_repl_offset:{}",
                    role, &master_replid, &master_repl_offset);
                Ok(Some(RespType::BulkStr { length: msg.len(), value: Some(msg) }))
            }
        } else {
            Ok(None)
        }
    }

    fn cmd_psync(&mut self, id: String, offset: i64, client_id: u64) -> Result<Option<RespType>, RespType> {
        if id == "?" && offset == -1 {
            // If sending client is slave
            let stats = AppStates::get().host_stats.as_ref().unwrap();
            // let id = stats.id.read().unwrap();
            let id = "8371b4fb1155b71f4a04d3e1bc3e18c4a990aeeb"; // TODO: for testing only
            let offset = stats.offset.load().max(0);
            let msg = format!("{} {} {}", &KW_FULLRESYNC, &id, &offset);
            self.response_queue.push((client_id, RespType::SimpleStr(Some(msg)).to_bytes().unwrap()));

            // Example
            let empty_rdb_b64 = "UkVESVMwMDEx+glyZWRpcy12ZXIFNy4yLjD6CnJlZGlzLWJpdHPAQPoFY3RpbWXCbQi8ZfoIdXNlZC1tZW3CsMQQAPoIYW9mLWJhc2XAAP/wbjv+wP9aog==";
            let empty_rdb = STANDARD.decode(empty_rdb_b64).unwrap();
            self.response_queue.push((client_id, RespType::RDB(Some(empty_rdb))
                .to_bytes().unwrap()));
            
            // Set client as slave
            println!("Set client {} as slave", &client_id);
            let _ = ClientTable::get().borrow_mut().set_slave(client_id);

            Ok(None)
        } else {
            Ok(None)
        }
    }

    fn cmd_replconf(&mut self, arg: CmdArg, client_id: u64, epoll_fd: Option<i32>) -> Result<Option<RespType>, RespType> {
        match arg {
            CmdArg::GETACK(s) => {
                // Slave receives this after handshake established
                if s == "*" {
                    // Response REPLCONF ACK <offset>
                    let mut arr = RespType::Array { length: 3, value: None };
                    let resp_replconf = RespType::BulkStr { length: KW_REPLCONF.len(), value: Some(KW_REPLCONF.to_string()) };
                    let resp_ack = RespType::BulkStr { length: KW_ACK.len(), value: Some(KW_ACK.to_string()) };
                    let offset = AppStates::get().host_stats.as_ref().unwrap().offset.load().max(0).to_string();
                    let resp_offset = RespType::BulkStr { length: offset.len(), value: Some(offset) };
                    arr.add_item(resp_replconf);
                    arr.add_item(resp_ack);
                    arr.add_item(resp_offset);

                    // After this, server starts counting for bytes from master
                    AppStates::get().host_stats.as_ref().unwrap().start_bytes_count();
                    Ok(Some(arr))
                } else {
                    Ok(None)
                }
            },
            CmdArg::ListeningPort(_) => {
                Ok(Self::response_ok())
            },
            CmdArg::Capa(_) => {
                Ok(Self::response_ok())
            },
            CmdArg::ACK(offset) => {
                // This is when master receive ACK from slave
                // Make sure that slave sends this only when receive GETACK from master
                // that way slave was certainly registered in backlog_wait
                
                // client_id here is actually slave_id
                // so we need to get client_id through backlog_ack
                let slave_id = client_id;

                // List of client sending WAIT
                // let client_ids = self.registry.backlog_ack.get(&slave_id);

                // Make sure that master has add slave to backlog_ack when receiving WAIT
                let mut slave_client_done: Vec<u64> = Vec::new();
                let mut client_done: Vec<u64> = Vec::new();
                if let Some(client_ids) = self.registry.backlog_ack.get(&slave_id) {
                    for id in client_ids {
                        let timestamp = self.registry.backlog_wait.get(&id).unwrap();
                        match self.registry.store.get_mut(timestamp) {
                            Some(RequestEntry::Wait { required_count, actual_count, offset_snapshot, .. }) => {
                                // Check if slave offset >= snapshot offset 
                                if offset >= *offset_snapshot {
                                    actual_count.set(actual_count.get() + 1);

                                    // Slave fullfilled ACK to a client, must be removed from list
                                    // saved to remove later
                                    slave_client_done.push(*id);
                                };

                                // If required count reaches, response to client
                                if actual_count.get() >= *required_count {
                                    // Response to client
                                    let resp_count = RespType::Integer(Some(actual_count.get() as i64));
                                    self.response_queue.push((*id, resp_count.to_bytes().unwrap()));

                                    // 
                                    client_done.push(*id);
                                }
                            },
                            _ => {}
                        }
                    };
                };
                
                // Remove slave fullfilling ACT to a client
                let mut slave_done = false;
                if let Some(client_list) = self.registry.backlog_ack.get_mut(&slave_id) {
                    for i in slave_client_done {
                        client_list.retain(|x| *x != i);
                    }
                    slave_done = client_list.is_empty();
                };

                // Remove slave if fullfilled all clients
                if slave_done {
                    self.registry.backlog_ack.remove(&slave_id);
                };

                // Remove a client request if WAIT count is fullfilled
                let mut timestamp_done: Vec<u64> = Vec::new();
                client_done.iter().for_each(|i| {
                    let timestamp = self.registry.backlog_wait.get(i).unwrap();
                    timestamp_done.push(*timestamp);
                    
                    // Re-register fd for client
                    let _ = add_interest(epoll_fd.unwrap(), *i as i32, get_epoll_event_read(*i));
                });
                timestamp_done.iter().for_each(|t| {self.registry.remove(t);});
                
                Ok(None)
            },
            _ => Ok(None)
        } 
    }

    fn cmd_wait(&mut self, count: u64, timeout_ms: Option<u64>, client_id: u64, epoll_fd: Option<i32>)
        -> Result<Option<RespType>, RespType> {
        // 3 cases of WAIT:
        // - WAIT 0 0: return immediately, no blocking
        // - WAIT N 0: block indefinitely till N ACK catching up
        // - WAIT N T: block till N ACK catching up or expire in T
        // Return the RESP integer nbr of slaves catching up

        if count == 0 {
            // First case  
            let resp_count = RespType::Integer(Some(0));
            return Ok(Some(resp_count))
        };
    
        // The block indefinitely or not is implemented in registry.insert
        
        // Get offset snapshot
        let offset_snapshot = AppStates::get().host_stats.as_ref().unwrap().offset.load();

        // Send ACK to all slave
        let mut arr = RespType::Array { length: 3, value: None };
        let resp_replconf = RespType::BulkStr { length: KW_REPLCONF.len(), value: Some(KW_REPLCONF.to_string()) };
        let resp_getack = RespType::BulkStr { length: KW_GETACK.len(), value: Some(KW_GETACK.to_string()) };
        let resp_asterik = RespType::BulkStr { length: 1, value: Some("*".to_string()) };
        arr.add_item(resp_replconf);
        arr.add_item(resp_getack);
        arr.add_item(resp_asterik);
        
        let mut slave_ids: Vec<u64> = Vec::new();
        match ClientTable::get().borrow().slaves.as_ref() {
            Some(h) => {
                for k in h.iter() {
                    self.response_queue.push((*k, arr.to_bytes().unwrap()));
                    slave_ids.push(*k);
                }
            },
            None => {}
        };

        let (timestamp, deadline) = Self::get_deadline(timeout_ms);
        let actual_count: Rc<Cell<u64>> = Rc::new(Cell::new(0));
        let count_ref = actual_count.clone();
        let deadline_task = Box::new(move |handler: &mut CmdHandler| {
            // Send response to client
            let resp_count = RespType::Integer(Some(count_ref.get() as i64));
            handler.response_queue.push((client_id, resp_count.to_bytes().unwrap()));

            // Remove client from registry
            handler.registry.remove(&timestamp);

            // Re-register epoll
            let _ = add_interest(epoll_fd.unwrap(), client_id as i32, get_epoll_event_read(client_id));
        });
        
        // Register the entry which store the ACK data
        
        let request_entry = RequestEntry::Wait {
            client_id,
            required_count: count,
            actual_count: actual_count,
            deadline,
            offset_snapshot,
            slave_ids,
            deadline_task
        };
        self.registry.insert(timestamp, deadline, request_entry);

        // Remove current client from epoll wait queue till response
        let _ = remove_interest(epoll_fd.unwrap(), client_id as i32);
        
        Ok(None)
    }
}
