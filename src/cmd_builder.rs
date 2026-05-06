use std::collections::{BTreeMap, HashSet};
use std::collections::{HashMap, VecDeque, hash_map::Entry };
use std::string::ParseError;
use std::time::{SystemTime, UNIX_EPOCH};
use std::u64;

use crate::resp::{ RespType, RespValue };
use crate::epoll::timer_create_event;


// Command Keyword
const KW_PING: &str = "PING";
pub const KW_PONG: &str = "PONG";
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
const KW_TYPE: &str = "TYPE";
const KW_XADD: &str = "XADD";

//-------Customed error for command construction and handling
#[derive(Debug)]
pub enum CmdError {
    InvalidArgument(String),
    MissingArgument(String),
    ParseError(String),
    NoCmdError,
    ParseIntError(String),
    UnsupportedCmdStructure,
    UnsupportedCommand(String),
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
            Self::UnsupportedCommand(msg) => write!(f, "Key {} does not support this command", msg),
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
    LPUSH { key: String, value: VecDeque<String> },
    LLEN(String),
    LPOP{ key: String, length: Option<usize> },
    BLPOP{ key: String, timeout_ms: Option<i64> },
    TYPE(String),
    XADD{ key: String, id: String, value: Vec<String>},
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
        
        let mut list_values: VecDeque<String> = VecDeque::new();
        while !values.is_empty() {
            // Pop from values, extract String, push to list_values
            let v = values.pop_front()
                .ok_or(CmdError::MissingArgument("No value provided for LPUSH".to_string()))?
                .get_value().unwrap()
                .str().unwrap();
            list_values.push_back(v);
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
            ).map(|x| (x * 1000.0) as i64)?;
        
        if timeout_ms < 0 {
            Err(CmdError::InvalidArgument("expiration".to_string()))
        } else if timeout_ms == 0 {
            Ok(Self::BLPOP { key, timeout_ms: None })
        } else {
            Ok(Self::BLPOP { key, timeout_ms: Some(timeout_ms) })
        }
    }
    
    fn ktype(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let key: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No key provided for TYPE".to_string()))?
            .get_value().unwrap().str().unwrap();
        Ok(Self::TYPE(key))
    }

    fn xadd(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let key: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No key provided for XADD".to_string()))?
            .get_value().unwrap().str().unwrap();
        let id: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No id provided for XADD".to_string()))?
            .get_value().unwrap().str().unwrap();
        
        let mut value: Vec<String> = Vec::new();
        for v in values {
            value.push(v.get_value().unwrap().str().unwrap());
        };
        Ok(Self::XADD { key, id, value })
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
                                        },
                                        s if s == KW_TYPE.to_string() => {
                                            return Self::ktype(v)
                                        },
                                        s if s == KW_XADD.to_string() => {
                                            return Self::xadd(v)
                                        },
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
