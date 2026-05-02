use std::collections::{HashMap, VecDeque, hash_map::Entry };
use std::string::ParseError;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::resp::{ RespType, RespValue };


// Command Keyword
const KW_PING: &str = "PING";
const KW_PONG: &str = "PONG";
const KW_ECHO: &str = "ECHO";
const KW_SET: &str = "SET";
const KW_GET: &str = "GET";
const KW_PX: &str = "PX";
const KW_EX: &str = "EX";
const KW_RPUSH: &str = "RPUSH";
const KW_LRANGE: &str = "LRANGE";
const KW_LPUSH: &str = "LPUSH";
const KW_LLEN: &str = "LLEN";
const KW_LPOP: &str = "LPOP";
const KW_BLPOP: &str = "BLPOP";

//-------Customed error for command construction
#[derive(Debug)]
pub enum CmdError {
    InvalidArgument(String),
    MissingArgument(String),
    ParseError(String),
    NoCmdError,
    ParseIntError(String),
    UnsupportedCmdStructure,
}

impl std::fmt::Display for CmdError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InvalidArgument(msg) => write!(f, "{}", msg),
            Self::MissingArgument(msg) => write!(f, "{}", msg),
            Self::ParseError(msg) => write!(f, "Error parsing {}", msg),
            Self::NoCmdError => write!(f, "No command found"),
            Self::ParseIntError(msg) => write!(f, "Error parsing int value {}", msg),
            Self::UnsupportedCmdStructure => write!(f, "Unsupported command structure"),
        }
    }
}


//-------Command, struct and parser
#[derive(Debug)]
pub enum Cmd {
    PING, 
    ECHO(String),
    SET { key: String, value: String, opt: Option<CmdOption>  },
    GET { key: String },
    RPUSH { key: String, value: Vec<String> },
    LRANGE{ key: String, start: i64, stop: i64},
    LPUSH { key: String, value: Vec<String> },
    LLEN(String),
    LPOP{ key: String, length: Option<usize> },
    BLPOP{ key: String, timeout_ms: Option<i64> },
}

impl Cmd {
    fn ping() -> Result<Self, CmdError> { Ok(Self::PING) }
    fn echo(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let s: String = values.pop_front()
            .ok_or(CmdError::MissingArgument(
                    "No argument provided for ECHO".to_string()))?
            .get_value()
            .ok_or(CmdError::ParseError("ECHO".to_string()))?
            .str()
            .ok_or(CmdError::ParseError("ECHO".to_string()))?;
        return Ok(Self::ECHO(s));
    }
    
    fn set(mut values: VecDeque<RespType> ) -> Result<Self, CmdError> {
        let key: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No key provided for SET".to_string()))?
            .get_value().unwrap()
            .str().unwrap();

        let value: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No value provided for SET".to_string()))?
            .get_value().unwrap()
            .str().unwrap();

        match values.pop_front() {
            // Having option
            Some(o) => {
                let expire_key: String = o.get_value().unwrap().str().unwrap();        
                let expire_value: u64 = match values.pop_front() {
                    Some(o) => {
                        // TODO: handle conversion error
                        o.get_value().unwrap().str().unwrap()
                            .parse::<u64>()
                            .map_err(|_| CmdError::ParseError("expiration value".to_string()))?
                    },
                    None => return Err(CmdError::MissingArgument("No expiration provided".to_string()))
                };
                let opt = CmdOption::set(expire_key, expire_value)
                    .ok_or(CmdError::ParseIntError("SET".to_string()))?;
                Ok(Self::SET{ key: key, value: value, opt: Some(opt) })
            },
            // Have no option
            None => Ok(Self::SET{ key: key, value: value, opt: None })
        }
    }

    fn get(mut values: VecDeque<RespType>) -> Result<Self, CmdError>{
        let key: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No key provided for GET".to_string()))?
            .get_value().unwrap().str().unwrap();
        Ok(Cmd::GET{ key })
    }

    fn rpush(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let key: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No key provided for RPUSH".to_string()))?
            .get_value().unwrap().str().unwrap();
        
        let mut list_values: Vec<String> = Vec::new();
        while !values.is_empty() {
            // Pop from values, extract String, push to list_values
            let v = values.pop_front()
                .ok_or(CmdError::MissingArgument("No value provided for RPUSH".to_string()))?
                .get_value().unwrap()
                .str().unwrap();
            list_values.push(v);
        };
        Ok(Cmd::RPUSH { key, value: list_values })
    }
    
    fn lrange(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let key: String = values.pop_front()
            .ok_or(
                CmdError::MissingArgument(
                    "No key provided for LRANGE".to_string()))?
            .get_value().unwrap().str().unwrap();
        let start: i64 = values.pop_front()
            .ok_or(
                CmdError::MissingArgument(
                    "No start index provided for LRANGE".to_string()))?
            .get_value().unwrap().str().unwrap()
            .parse().map_err(
                |_| CmdError::InvalidArgument(
                    "start index for LRANGE".to_string()))?;
        let stop: i64 = values.pop_front()
            .ok_or(
                CmdError::MissingArgument(
                    "No end index provided for LRANGE".to_string()))?
            .get_value().unwrap().str().unwrap()
            .parse().map_err(
                |_| CmdError::InvalidArgument(
                    "end index for LRANGE".to_string()))?;
        Ok(Self::LRANGE { key, start, stop })
    }
    
    fn lpush(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let key: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No key provided for LPUSH".to_string()))?
            .get_value().unwrap().str().unwrap();
        
        let mut list_values: Vec<String> = Vec::new();
        while !values.is_empty() {
            // Pop from values, extract String, push to list_values
            let v = values.pop_front()
                .ok_or(CmdError::MissingArgument("No value provided for LPUSH".to_string()))?
                .get_value().unwrap()
                .str().unwrap();
            list_values.push(v);
        };
        Ok(Cmd::LPUSH { key, value: list_values })
    }

    fn llen(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let key: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No key provided for LLEN".to_string()))?
            .get_value().unwrap().str().unwrap();
        Ok(Cmd::LLEN(key))
    }

    fn lpop(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let key: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No key provided for LPOP".to_string()))?
            .get_value().unwrap().str().unwrap();
        
        let length: Option<usize> = match values.pop_front() {
            Some(s) => {
                match s.get_value().unwrap().str().unwrap()
                    .parse::<usize>() {
                    Ok(x) => Some(x),
                    Err(_) => return
                        Err(CmdError::InvalidArgument("length for LPOP".to_string()))
                    }
            },
            None => None
        };

        Ok(Cmd::LPOP{ key, length })
    }

    fn blpop(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let key: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No key provided for BLPOP".to_string()))?
            .get_value().unwrap().str().unwrap();
        
        let timeout_ms: i64 = values.pop_front()
            .ok_or(CmdError::MissingArgument("No timeout provided for BLPOP".to_string()))?
            .get_value().unwrap().str().unwrap()
            .parse::<f64>()
            .map_err(
                |_| CmdError::ParseError("timeout for BLPOP".to_string())
            )? as i64;
        
        if timeout_ms == 0 {
            Ok(Self::BLPOP { key, timeout_ms: None })
        } else {
            Ok(Self::BLPOP { key, timeout_ms: Some(timeout_ms) })
        }
    }

    pub fn from_resp(resp_type: RespType) -> Result<Self, CmdError> {
        // Instantiate Cmd from RespType
        match resp_type {
            RespType::Array{ length, value } => {
                // Iterate through the array to construct Cmd
                // A command is always in array form
                if length == 0 { return Err(CmdError::NoCmdError) };

                // First item must be cmd type
                if  let Some(mut v) = value {
                    match v.pop_front() {
                        Some(o) => {
                            match o {
                                RespType::BulkStr { length, value } => {
                                    if length == 0 { return Err(CmdError::NoCmdError) };

                                    match value.unwrap().to_uppercase() {
                                        s if s == KW_PING.to_string() => {
                                            return Self::ping();
                                        },
                                        s if s == KW_ECHO.to_string() => {
                                            return Self::echo(v);
                                        },
                                        s if s == KW_SET.to_string() => {
                                            return Self::set(v);
                                        },
                                        s if s == KW_GET.to_string() => {
                                            return Self::get(v);
                                        },
                                        s if s == KW_RPUSH.to_string() => {
                                            return Self::rpush(v);
                                        },
                                        s if s == KW_LRANGE.to_string() => {
                                            return Self::lrange(v);
                                        },
                                        s if s == KW_LPUSH.to_string() => {
                                            return Self::lpush(v);
                                        },
                                        s if s == KW_LLEN.to_string() => {
                                            return Self::llen(v);
                                        },
                                        s if s == KW_LPOP.to_string() => {
                                            return Self::lpop(v);
                                        },
                                        s if s == KW_BLPOP.to_string() => {
                                            return Self::blpop(v)
                                        }
                                        _ => return Err(
                                            CmdError::InvalidArgument("Invalid command".to_string()))
                                    } 
                                },
                                _ => return Err(CmdError::InvalidArgument("Invalid command".to_string()))
                            }            
                        },
                        None => return Err(CmdError::NoCmdError) 
                    };
                };
                return Err(CmdError::NoCmdError);
            },
            _ => return Err(CmdError::UnsupportedCmdStructure),
        }
    }
}

#[derive(Debug)]
pub enum CmdOption {
    EX(Option<u64>), // expire in x seconds
    PX(Option<u64>), // expire in x miliseconds
}

impl CmdOption {
    fn set(key: String, value: u64) -> Option<Self> {
        match Self::match_key(key)? {
            Self::EX(_) => Some(Self::EX(Some(value))),
            Self::PX(_) => Some(Self::PX(Some(value))),
        }
    }

    fn match_key(key: String) -> Option<Self> {
        match key.to_uppercase() {
            k if k == KW_EX => Some(Self::EX(None)),
            k if k == KW_PX => Some(Self::PX(None)),
            _ => return None
        }
    } 
}

// Stored value types in Cmd
#[derive(Debug, Clone)]
struct HashItem { value: String, expired_at: Option<u64> }

//---------Command handler, convert Cmd struct into action
#[derive(Debug)]
pub struct CmdHandler {
    pub response_queue: Vec<(u64, String)>,             // client_id, message
    hashes: HashMap<String, HashItem>,
    lists: HashMap<String, VecDeque<String>>,
    lists_queue: HashMap<String, VecDeque<(u64, u64)>>, // BLPOP queue for each list: 
                                                        // - list name
                                                        // - tuple of client id and associated deadline 
                                                        // no deadline = 0
    deadline_client: HashMap<u64, u64>,                 // map deadline - client
                                                        // deadline set or served is pop off this
    deadline_set: Option<(u64, u64)>,                   // once set a deadline pop off deadline_client and
                                                        // inserted here
    deadline_done: HashMap<u64, u64>,                   // fullfilled deadline 
}

// Thx to the loop nature, each BLPOP request has distinct timestamp

impl CmdHandler {
    pub fn new() -> Self{
        Self { 
            response_queue: Vec::new(),
            hashes: HashMap::new(),
            lists: HashMap::new(),
            lists_queue: HashMap::new(),
            deadline_client: HashMap::new(),
            deadline_set: None,
            deadline_done: HashMap::new(),
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
                }
            },
            Err(e) => Self::cmd_err(e.to_string())
        } 
    }
    
    // TODO: all these returns should be of Result<>
    fn now() -> u64 {
        SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap().as_millis() as u64
    }

    fn response_ok() -> Option<String> {
        RespType::SimpleStr(Some("OK".to_string())).serialize()
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
        if let Some(arg) = opt {
            let now = Self::now(); 

            let exp = match arg {
                CmdOption::EX(x) => {
                    Some(now / 1000 + x.unwrap())
                },
                CmdOption::PX(x) => {
                    Some(now + x.unwrap())
                }
            };

            self.hashes.insert(key, HashItem { value: value, expired_at: exp });
        } else {
            self.hashes.insert(key, HashItem { value: value, expired_at: None });
        }
        Self::response_ok()
    }

    fn cmd_get(&self, key: String) -> Option<String> {
        match self.hashes.get(&key) {
            Some(v) => {
                // Check for expiration
                let expired = v.expired_at.map_or(false, |x| Self::now() > x);
                let value = if expired { None } else { Some(v.value.clone()) };
                RespType::BulkStr{
                    length: value.as_ref().map_or(0, |s| s.len()),
                    value: value
                }.serialize()
            },
            // No key found
            None => RespType::BulkStr{ length: 0, value: None }.serialize(),
        } 
    }

    fn cmd_rpush(&mut self, key: String, value: Vec<String>) -> Option<String> {
        let list = self.lists.entry(key.clone()).or_insert_with(VecDeque::new);
        self.lists_queue.entry(key).or_insert_with(VecDeque::new);
        list.extend(value);
        RespType::Integer(Some(list.len() as i64)).serialize()
    }

    fn cmd_lrange(&self, key: String, start: i64, stop: i64) -> Option<String> {
        // At this phrase, start and stop are non negative
        // - min index = 0
        // - max index = len - 1
        
        // Check valid key First
        match self.lists.get(&key) {
            Some(list) => {
                // Edge case
                if start > stop && start > 0 && stop > 0 {
                    return RespType::Array{ length: 0, value: None}.serialize()
                };
                
                // VecDeque could wrap it self, so need this
                if start >= 0 && start as usize > list.len() - 1 {
                    return RespType::Array{ length: 0, value: None}.serialize()
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
                    RespType::Array{ length: output_len, value: None}.serialize()
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
            None => {
                RespType::Array{ length: 0, value: None }.serialize()
            }
        }
    }
    
    fn cmd_lpush(&mut self, key: String, value: Vec<String>) -> Option<String> {
        let list = self.lists.entry(key.clone()).or_insert_with(VecDeque::new);
        self.lists_queue.entry(key).or_insert_with(VecDeque::new);
        for v in value {
            list.push_front(v)
        };
        RespType::Integer(Some(list.len() as i64)).serialize()
    }

    fn cmd_llen(&self, key: String) -> Option<String> {
        match self.lists.get(&key) {
            Some(list) => {
                RespType::Integer(Some(list.len() as i64)).serialize()
            },
            None => {
                RespType::Integer(Some(0)).serialize()
            }
        }
    }
    
    fn cmd_lpop(&mut self, key: String, length: Option<usize>) -> Option<String> {
        match self.lists.get_mut(&key) {
            Some(list) => {
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
            None => {
                RespType::BulkStr { length: 0, value: None }.serialize()
            }
        }         
    }

    fn cmd_blpop(
        &mut self, 
        key: String, 
        timeout_ms: Option<i64>, 
        client_id: u64) -> Option<String> {
        // timeout is converted to deadline
        
        let deadline: u64 = match timeout_ms{
            Some(x) => {
                Self::now() + x as u64
            },
            None => {
                0
            } 
        };

        match self.lists.get_mut(&key) {
            Some(list) => {
                // If key-list exists, key-list queue must exists
                // as being created by rpush and lpush
                let list_queue = self.lists_queue.get_mut(&key).unwrap();
                if list_queue.is_empty() && !list.is_empty() {
                    // No other client in waiting list and item available, pop
                    let item = list.pop_front().unwrap();
                    let mut output = RespType::Array{ length: 2, value: None};
                    let list_name = RespType::BulkStr { length: key.len(), value: Some(key) };
                    let value = RespType::BulkStr { length: item.len(), value: Some(item) };
                    output.add_item(list_name);
                    output.add_item(value);
                    output.serialize()
                } else {
                    // Wait
                    self.deadline_client.insert(deadline, client_id);
                    self.lists_queue.get_mut(&key).unwrap().push_back((client_id, deadline));
                    None
                } 
            },
            None => {
                // No list exists, wait in queue for client
                self.lists_queue.entry(key.clone())
                    .or_insert_with(VecDeque::new).push_back((client_id, deadline));
                None
            }
        }
    }

    pub fn serve_blpop_queue(&mut self) {
        // Execute once at the end of each event loop 1,
        // matching BLPOP queue with available item,
        // other LPOP requests are served at client level 2
        for (k, queue) in self.lists_queue.iter_mut() {
            while let Some(list) = self.lists.get_mut(k) {
                if list.is_empty() || queue.is_empty() {
                    break
                };
                
                let (client_id, deadline) = queue.pop_front().unwrap();
                let item = list.pop_front().unwrap();
                
                // Construct response RESP
                let mut resp_response = RespType::Array { length: 2, value: None};
                let list_name = RespType::BulkStr { length: k.len(), value: Some(k.clone())};
                let value = RespType::BulkStr { length: item.len(), value: Some(item)};
                resp_response.add_item(list_name);
                resp_response.add_item(value);
                
                // Push response to output
                self.response_queue.push((client_id, resp_response.serialize().unwrap()));

                // push deadline to deadline_done
                // later when a timer fired, it will get the current dealine in deadline_set
                // and check whether this deadline has been served (check this hashmap)
                // prevent triggering timer when queue has been served
                self.deadline_done.insert(deadline, client_id);
            }    
        };
    }

    pub fn get_next_timeout(&mut self) -> Option<u64> {
        // Get the minimum deadline in deadline_client
        // converted to timeout
        let next_deadline = self.deadline_client.keys().min()?.clone();
        let client_id = self.deadline_client.get(&next_deadline).unwrap().clone();
        
        // Remove deadline as it will be set
        self.deadline_client.remove(&next_deadline);

        let now = Self::now();
        if next_deadline > Self::now() {
            self.deadline_set = Some((next_deadline, client_id));
            Some(next_deadline - now)
        } else {
            // Already expired, could be that timeout is set too short
            // push a message to response queue
            let msg = RespType::BulkStr { length: 0, value: None }.serialize()?;
            self.response_queue.push((client_id, msg));
            None
        }
    }

    pub fn process_deadline_served(&mut self) {
        // Get current deadline, the one just been fired
        match self.deadline_set {
            Some(x) => {
                let (deadline, client_id) = x;
                // Check whether this deadline has been served
                let done = self.deadline_done.get(&deadline).cloned();
                match done {
                    Some(_) => {
                        // Pop this out of deadline_done
                        self.deadline_done.remove(&deadline);
                    },
                    None => {
                        // Not served, response NullBulkStr
                        let msg = RespType::BulkStr { length: 0, value: None }
                            .serialize().unwrap();
                        self.response_queue.push((client_id, msg));
                    }
                }
            },
            None => {}
        }
    }
}
