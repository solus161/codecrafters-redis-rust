use std::collections::{BTreeMap, HashSet};
use std::collections::{HashMap, VecDeque, hash_map::Entry };
use std::string::ParseError;

use libc::write;

use crate::resp::{ RespType, RespValue };
use crate::epoll::timer_create_event;
use crate::cmd_builder::{ Cmd, CmdError, CmdOption, KW_PONG };
use crate::utils::now;

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

type Task = Box<dyn FnOnce(&mut CmdHandler) -> Option<String>>;

//---------Request registry: manage waiting request for CmdHandler
struct RequestEntry {
    pub client_id: u64,
    pub key: String,
    pub deadline: u64,
    pub backlog_task: Task,     // Task run when request fullfilled
    pub deadline_task: Task,    // Task run at deadline
}

struct RequestRegistry {
    // timestamp - request
    store: HashMap<u64, RequestEntry>,
    
    // key - queue of timestamp
    backlog: HashMap<String, VecDeque<u64>>,

    // deadline - timestamp
    deadline: BTreeMap<u64, u64>,
    timer_fd: i32,
}

impl RequestRegistry {
    pub fn new(timer_fd: i32) -> Self {
        Self {
            store: HashMap::new(),
            backlog: HashMap::new(),
            deadline: BTreeMap::new(),
            timer_fd,
        }
    }
    
    pub fn insert(
        &mut self, 
        timestamp: u64,
        client_id: u64, 
        key: String, 
        deadline: u64,
        backlog_task: Task,
        deadline_task: Task) {
        self.backlog.entry(key.clone())
            .or_insert_with(VecDeque::new).push_back(timestamp);
        if deadline > 0 {
            self.deadline.insert(deadline, timestamp);
            self.set_timer_fd();
        };
            
        self.store.insert(timestamp, RequestEntry {
            client_id, key, deadline, backlog_task, deadline_task
        });
    }
    
    pub fn remove(&mut self, timestamp: &u64) -> Option<RequestEntry> {
        let entry = self.store.remove(timestamp)?;
        if let Some(q) = self.backlog.get_mut(&entry.key) {
            // This will have no effect if backlog is pooped
            // when a backlog condition fullfilled
            q.retain(|&t| t!= *timestamp);
            if q.is_empty() {
                self.backlog.remove(&entry.key);
                self.set_timer_fd();
            }
        };
        self.deadline.remove(&entry.deadline);
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

    pub fn is_backlog_empty(&self, key: &str) -> bool {
        match self.backlog.get(key) {
            Some(list) => {
                list.is_empty()
            },
            None => false
        }
    }

    pub fn get_nearest_deadline(&self) -> Option<(&u64, &u64)> {
        self.deadline.first_key_value() 
    }
}

//---------Command handler, convert Cmd struct into action
pub struct CmdHandler {
    // client_id, message
    pub response_queue: Vec<(u64, String)>,
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

    pub fn handle(&mut self, cmd: Result<Cmd, CmdError>, client_id: u64) -> Option<String> {
        match cmd {
            Ok(c) => {
                match c {
                    Cmd::PING => Self::cmd_ping(),
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
                }
            },
            Err(e) => Self::cmd_err(e.to_string())
        } 
    }

    fn extract_deadline(opt: Option<CmdOption>) -> Option<u64> {
        match opt {
            Some(arg) => {
                let now = now(); 
                match arg {
                    CmdOption::EX(x) => Some(now / 1000 + x.unwrap()),
                    CmdOption::PX(x) => Some(now + x.unwrap()),
                }
            },
            None => None
        }
    }
    
    fn get_deadline(timeout_ms: Option<i64>) -> (u64, u64) {
        let now = now();
        match timeout_ms{
            Some(timeout) => {
                (now, now + timeout as u64)
            },
            None => (now, 0 as u64)
        }
    }

    fn get_null_array() -> Option<String> {
        RespType::Array { length: 0, value: None }.serialize()
    }

    pub fn callback_deadline_expire(&mut self) {
        // A callback fired when deadline expired (triggere by timer fd):
        // Execute callback in deadline_task
        let timestamp = *self.registry.get_nearest_deadline().unwrap().1;
        let entry = self.registry.remove(&timestamp).unwrap();
        (entry.deadline_task)(self);
    }

    fn response_ok() -> Option<String> {
        RespType::SimpleStr(Some("OK".to_string())).serialize()
    }
    
    pub fn serve_queue(&mut self) {
        // Execute once at the end of main each event loop,
        // matching BLPOP queue with available item,
        // other LPOP requests are served at request time at client loop
        let mut timestamps: Vec<u64> = Vec::new();

        for (k, v) in self.registry.backlog.iter() {
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
                tasks.push(entry.backlog_task);
            }
        }
        for task in tasks {
            task(self);
        }
    }

    fn cmd_ping() -> Option<String> {
        RespType::SimpleStr(Some(KW_PONG.to_string())).serialize()
    }

    fn cmd_echo(s: String) -> Option<String> {
        RespType::BulkStr{ length: s.len(), value: Some(s) }.serialize() 
    }

    fn cmd_err(s: String) -> Option<String> {
        RespType::SimpleStr(Some(s)).serialize()
    }

    fn cmd_set(&mut self, key: String, value: String, opt: Option<CmdOption>) -> Option<String> {
        // Extract cmd for handling instruction
        let exp = Self::extract_deadline(opt); 
        self.data.insert(key, StoreItem {
            value: StoreValue::Str(value), expired_at: exp});
        Self::response_ok()
    }

    fn cmd_get(&self, key: String) -> Option<String> {
        match self.data.get(&key) {
            Some(item) => match &item.value {
                StoreValue::Str(s) => {
                    // Check for expiration
                    // TODO:: implement removed at expiration
                    let expired = item.expired_at.map_or(false, |x| now() > x);
                    let value = if expired { None } else { Some(s.clone()) };
                    RespType::BulkStr{
                        length: value.as_ref().map_or(0, |s| s.len()),
                        value: value
                    }.serialize()
                },
                _ => RespType::Error(
                    Some(CmdError::UnsupportedCommand(key).to_string()))
                    .serialize()
            },
            // No key found
            None => RespType::BulkStr{ length: 0, value: None }.serialize(),
        } 
    }

    fn cmd_rpush(&mut self, key: String, value: Vec<String>) -> Option<String> {
        let item = self.data.entry(key.clone()).or_insert(
            StoreItem {
                value: StoreValue::List(VecDeque::new()),
                expired_at: None });
        
        match &mut item.value {
            StoreValue::List(list) => {
                list.extend(value);
                RespType::Integer(Some(list.len() as i64)).serialize()
            },
            _ =>  RespType::Error(
                Some(CmdError::UnsupportedCommand(key).to_string())
            ).serialize()
        }
    }

    fn cmd_lrange(&self, key: String, start: i64, stop: i64) -> Option<String> {
        // Check valid key First
        match self.data.get(&key) {
            Some(item) => {
                match &item.value {
                    StoreValue::List(list) => {
                        // Edge case
                        if start > stop && start > 0 && stop > 0 {
                            return RespType::Array{ length: 0, value: Some(VecDeque::new())}.serialize()
                        };
                        
                        // VecDeque could wrap it self, so need this
                        if start >= 0 && start as usize > list.len() - 1 {
                            return RespType::Array{ length: 0, value: Some(VecDeque::new())}.serialize()
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
                            RespType::Array{
                                length: output_len, 
                                value: Some(VecDeque::new())
                            }.serialize()
                        } else {
                            let mut output = RespType::Array{
                                length: output_len as usize,
                                value: Some(VecDeque::<RespType>::new())};
                            
                            for i in list.iter().skip(min_index).take(output_len){
                                output.add_item(RespType::BulkStr { length: i.len(), value: Some(i.clone()) }); 
                            };
                            output.serialize()
                        }
                    },
                    _ => RespType::Error(
                        Some(CmdError::UnsupportedCommand(key).to_string())
                    ).serialize()
                }
            },
            None => {
                RespType::Array{ length: 0, value: Some(VecDeque::new()) }.serialize()
            }
        }
    }
    
    fn cmd_lpush(&mut self, key: String, value: VecDeque<String>) -> Option<String> {
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
                RespType::Integer(Some(list.len() as i64)).serialize()
            },
            _ => RespType::Error(
                Some(CmdError::UnsupportedCommand(key).to_string())
            ).serialize()
        }
        
    }

    fn cmd_llen(&self, key: String) -> Option<String> {
        match self.data.get(&key) {
            Some(item) => {
                match &item.value {
                    StoreValue::List(list) => {
                        RespType::Integer(Some(list.len() as i64)).serialize()
                    },
                    _ => {
                        RespType::Error(
                            Some(CmdError::UnsupportedCommand(key).to_string())
                        ).serialize()
                    }
                }
            },
            None => RespType::Integer(Some(0)).serialize() 
        }
    }
    
    fn cmd_lpop(&mut self, key: String, length: Option<usize>) -> Option<String> {
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
                                output.serialize()
                            },
                            None => {
                                match list.pop_front() {
                                    Some(s) => {
                                        RespType::BulkStr {
                                            length: s.len(), value: Some(s)}.serialize()
                                    },
                                    None => {
                                        RespType::BulkStr {
                                            length: 0, value: None }.serialize()
                                    }
                                }
                            }
                        }
                    },
                    _ => {
                        RespType::Error(
                            Some(CmdError::UnsupportedCommand(key).to_string())
                        ).serialize()
                    }
                }
            },
            None => {
                RespType::BulkStr { length: 0, value: None }.serialize()
            }
        }         
    }

    fn cmd_blpop(
        &mut self, 
        key: String, 
        timeout_ms: Option<i64>, 
        client_id: u64) -> Option<String> 
    {
        let (timestamp, deadline) = Self::get_deadline(timeout_ms);
        let key1 = key.clone();
        println!("BLPOP at {}, key {}, timeout ms {:?}", deadline, &key, timeout_ms);

        // Task to run when item available to pop
        // Checking for key and type must be done by parent calls this task
        let backlog_task = Box::new(move |handler: &mut CmdHandler| {
            // Task triggered when item available
            println!("Running backlog task for BLPOP at {}", deadline);
            
            let popped = handler.data.get_mut(&key)
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
                let resp_key = RespType::BulkStr { length: key.len(), value: Some(key) };
                let resp_item = RespType::BulkStr { length: item.len(), value: Some(item) };
                output.add_item(resp_key);
                output.add_item(resp_item);
                handler.response_queue.push((client_id, output.serialize().unwrap()));
                println!("Push to response queue: , {:?}", &handler.response_queue.last().unwrap());

                // Disable deadline
                handler.registry.remove(&timestamp);
            };
            None
        });

        let deadline_task = Box::new(move |handler: &mut CmdHandler| {
            let msg = Self::get_null_array().unwrap();
            handler.response_queue.push((client_id, msg));
            println!(
                "Push to response queue: {:?}", 
                &handler.response_queue.last().unwrap());
            handler.registry.remove(&timestamp);
            None
        });
 
        match self.data.get_mut(&key1) {
            // Key exists
            Some(item) => {
                match &mut item.value {
                    StoreValue::List(list) => {
                        // If key-list exists, key-backlog queue must exists
                        // as being created by rpush and lpush
                        // let backlog = self.backlog.get_mut(&key1).unwrap();
                        // if backlog.is_empty() && !list.is_empty() {
                        if self.registry.is_backlog_empty(&key1) && !list.is_empty() {
                            // No other client in waiting list and item available, pop
                            let item = list.pop_front().unwrap();
                            let mut output = RespType::Array{ length: 2, value: None};
                            let list_name = RespType::BulkStr { length: key1.len(), value: Some(key1) };
                            let value = RespType::BulkStr { length: item.len(), value: Some(item) };
                            output.add_item(list_name);
                            output.add_item(value);
                            output.serialize()
                        } else {
                            // Wait
                            self.registry.insert(
                                timestamp, client_id, key1, deadline, backlog_task, deadline_task);
                            None
                        }
                    },
                    _ => RespType::Error(
                        Some(CmdError::UnsupportedCommand(key1).to_string())
                    ).serialize()
                }
            },
            None => {
                // No key-list exists, also no key-backlog exists
                // wait in queue for client
                self.registry.insert(
                    timestamp, client_id, key1, deadline, backlog_task, deadline_task);
                None
            }
        }
    }

    fn cmd_type(&self, key: String) -> Option<String> {
        let ktype = match self.data.get(&key) {
            Some(item) => {
                item.value.get_type().to_string()
            },
            None => "none".to_string()
        };
        RespType::SimpleStr(Some(ktype)).serialize()
    }

    fn cmd_xadd(
        &mut self,
        key: String,
        id: (Option<u64>, Option<u64>), 
        // For this stage, id is explicitely declare
        // TODO: handle case id = * and id = 123-*
        value: Vec<String>) -> Option<String> {
        self.data.entry(key.clone()).or_insert(StoreItem {
            value: StoreValue::Stream(BTreeMap::new()),
            expired_at: None });

        let id_valid = match &self.data.get(&key).unwrap().value {
            StoreValue::Stream(b) => {
                let ts = id.0.unwrap();
                let id = id.1.unwrap();

                if ts == 0 && id == 0 {
                    return RespType::Error(Some(
                        "ERR The ID specified in XADD must be greater than 0-0".to_string())
                    ).serialize()
                };

                match b.last_key_value() {
                    Some((k, _)) => {
                        let (ts_last, id_last) = *k;
                        ts > ts_last || (ts == ts_last && id > id_last) 
                    },
                    None => true,
                }
            },
            _ => {
                return RespType::Error(Some(
                    CmdError::UnsupportedCommand(key).to_string())
                ).serialize();
            }
        };

        if id_valid {
            let stream_id = (id.0.unwrap(), id.1.unwrap());
            match &mut self.data.get_mut(&key).unwrap().value {
                StoreValue::Stream(btree) => {
                    btree.insert(stream_id, value);
                    let msg = format!("{}-{}", stream_id.0, stream_id.1);
                    RespType::BulkStr { length: msg.len(), value: Some(msg) }
                        .serialize()
                },
                _ => {
                    RespType::Error(Some(
                        CmdError::UnsupportedCommand(key).to_string())
                    ).serialize()
                }
           }
        } else {
            let msg = "ERR The ID specified in XADD is equal or smaller than the target stream top item";
            return RespType::Error(Some(
                msg.to_string())
            ).serialize()
        }
    }    
}
