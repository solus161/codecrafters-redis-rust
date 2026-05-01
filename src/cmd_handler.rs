use std::collections::{HashMap, VecDeque, hash_map::Entry };
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
                                        _ => return Err(CmdError::InvalidArgument("Invalid command".to_string()))
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
    hashes: HashMap<String, HashItem>,
    lists: HashMap<String, VecDeque<String>>,
}

impl CmdHandler {
    pub fn new() -> Self{
        Self { 
            hashes: HashMap::new(),
            lists: HashMap::new(),
        }
    }

    pub fn handle(&mut self, cmd: Result<Cmd, CmdError>) -> Option<String> {
        match cmd {
            Ok(c) => {
                match c {
                    Cmd::PING => Self::cmd_ping(),
                    Cmd::ECHO(s) => Self::cmd_echo(s),
                    Cmd::SET{ key, value, opt } => self.cmd_set(key, value, opt),
                    Cmd::GET{ key } => self.cmd_get(key),
                    Cmd::RPUSH{ key, value } => self.cmd_rpush(key, value),
                    Cmd::LRANGE { key, start, stop } => self.cmd_lrange(key, start, stop),
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
        let list = self.lists.entry(key).or_insert_with(VecDeque::new);
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
                println!("start {} stop {} start_abs {} stop_abs {} min {} max {}", start, stop, start_abs, stop_abs, min_index, max_index);
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
}
