use std::collections::{BTreeMap, HashSet};
use std::collections::{HashMap, VecDeque, hash_map::Entry };
use std::string::ParseError;
use std::time::{SystemTime, UNIX_EPOCH};
use std::u64;

use crate::resp::{ ParseStatus, RespType, RespValue };
use crate::epoll::timer_create_event;
use crate::utils::now;


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
const KW_XRANGE: &str = "XRANGE";
const KW_XREAD: &str = "XREAD";
const KW_STREAMS: &str = "STREAMS";
const KW_COUNT: &str = "COUNT";
const KW_BLOCK: &str = "BLOCK";
const KW_INCR: &str = "INCR";
const KW_MULTI: &str = "MULTI";
pub const KW_QUEUED: &str = "QUEUED";
const KW_EXEC: &str = "EXEC";
const KW_DISCARD: &str = "DISCARD";
const KW_WATCH: &str = "WATCH";
const KW_UNWATCH: &str = "UNWATCH";
const KW_INFO: &str = "INFO";
pub const KW_REPLICATION: &str = "REPLICATION";

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
    UnprocessableError(String),
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
            Self::UnprocessableError(msg) => write!(f, "{}", msg),
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
    XADD{ key: String, id: (Option<u64>, Option<u64>), value: Vec<String>},
    XRANGE{ key: String, start: (u64, u64), end: (u64, u64)},
    XREAD{ count: Option<u64>, block_ms: Option<u64>, stream: Vec<(String, u64, u64)>, },
    INCR(String),
    MULTI,
    EXEC,
    DISCARD,
    WATCH(Vec<String>),
    UNWATCH,
    INFO(String),
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
                    "Error while parsing start index for LRANGE".to_string()))?;
        let stop: i64 = values.pop_front()
            .ok_or(
                CmdError::MissingArgument(
                    "No end index provided for LRANGE".to_string()))?
            .get_value().unwrap().str().unwrap()
            .parse().map_err(
                |_| CmdError::InvalidArgument(
                    "Error while parsing end index for LRANGE".to_string()))?;
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
                        Err(CmdError::InvalidArgument("Error while parsing length for LPOP".to_string()))
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
            Err(CmdError::InvalidArgument("Error while parsing expiration for BLPOP".to_string()))
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

    fn _parse_stream_id_xadd(value: String) -> Result<(Option<u64>, Option<u64>), CmdError> {
        // Parse timestamp id of stream
        let id: (Option<u64>, Option<u64>);
        if value == "*".to_string() {
            id = (None, None);
        
        } else {
            let vec_splitted: Vec<&str> = value.split('-').collect();
            match vec_splitted.as_slice() {
                [t, i] => {
                    let ts: Option<u64> = Some(t.parse::<u64>().map_err(
                        |_| CmdError::InvalidArgument("Error while parsing stream id for XADD".to_string())
                    )?);

                    if *i == "*".to_string() {
                        id = (ts, None);  
                    } else {
                        let idx: Option<u64> = Some(i.parse::<u64>().map_err(
                            |_| CmdError::InvalidArgument("Error while parsing stream id for XADD".to_string())
                        )?);
                        id = (ts, idx)
                    }
                },
                _ => return Err(CmdError::InvalidArgument("Error while parsing stream id for XADD".to_string()))
            };
        };
        Ok(id)
    }

    fn xadd(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let key: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No key provided for XADD".to_string()))?
            .get_value().unwrap().str().unwrap();
        let timestamp_id: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No timeid provided for XADD".to_string()))?
            .get_value().unwrap().str().unwrap();

        // Parse timestamp_id
        let id = Self::_parse_stream_id_xadd(timestamp_id)?; 
        let mut value: Vec<String> = Vec::new();
        for v in values {
            value.push(v.get_value().unwrap().str().unwrap());
        };
        Ok(Self::XADD { key, id, value })
    }

    fn _parse_stream_id(value: String, end: bool) -> Result<(u64, u64), CmdError> {
        let vec_splitted: Vec<&str> = value.split('-').collect();
        let id = match vec_splitted.as_slice() {
            [t, i] => {
                let ts: u64 = t.parse::<u64>().map_err(
                    |_| CmdError::InvalidArgument("Error while parsing stream id".to_string())
                )?;

                let idx: u64 = i.parse::<u64>().map_err(
                    |_| CmdError::InvalidArgument("Error while parsing stream id".to_string())
                )?;

                (ts, idx)
            },
            [t] => {
                let ts: u64 = t.parse::<u64>().map_err(
                    |_| CmdError::InvalidArgument("Error while parsing stream id".to_string())
                )?;
                
                if !end {
                    (ts, 0)
                } else {
                    (ts, u64::MAX)
                }
                
            }
            _ => return Err(CmdError::InvalidArgument("Error while parsing stream id".to_string()))
        };
        Ok(id)
    }
    
    fn _parse_stream_id_xrange(value: String, end: bool) -> Result<(u64, u64), CmdError> {
        // Parse timestamp id of stream
        if value == "-" {
            return Ok((0, 0))
        };
        
        if value == "+" {
            return Ok((u64::MAX, u64::MAX))
        };
        
        Self::_parse_stream_id(value, end)
    }

    fn _parse_stream_id_xread(value: String) -> Result<(u64, u64), CmdError> {
        if value == "$" {
            return Ok((u64::MAX, u64::MAX));
        };
        Self::_parse_stream_id(value, false)
    }

    fn xrange(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let key: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No key provided for XRANGE".to_string()))?
            .get_value().unwrap().str().unwrap();

        let start_id: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No start id provided for XRANGE".to_string()))?
            .get_value().unwrap().str().unwrap();
    
        let end_id: String = values.pop_front()
            .ok_or(CmdError::MissingArgument("No start id provided for XRANGE".to_string()))?
            .get_value().unwrap().str().unwrap();

        // Extract start id
        let start = Self::_parse_stream_id_xrange(start_id, false)?;
        let end = Self::_parse_stream_id_xrange(end_id, true)?;

        Ok(Self::XRANGE { key, start, end })
    }

    fn xread(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let mut count = None;
        let mut stream: Vec<(String, u64, u64)> = Vec::new();
        let mut block_ms: Option<u64> = None;
       
        let mut has_count = false;
        let mut has_block = false;

        let parse_u64_arg = |kw: &str, values: &mut VecDeque<RespType>|
            -> Result<u64, CmdError> {
            values.pop_front()
                .ok_or(CmdError::MissingArgument("No argument provided for COUNT".to_string()))?
                .get_value().unwrap().str().unwrap()
                .parse::<u64>().map_err( |_|
                    CmdError::ParseError(format!("value for {kw}"))
                )
        };
        
        // COUNT or BLOCK could come first
        loop {
            let first = values.pop_front()
                .ok_or(CmdError::MissingArgument("No argument provided for XREAD".to_string()))?
                .get_value().unwrap().str().unwrap().to_uppercase();
            if first == KW_COUNT {
                if has_count {
                    return Err(
                        CmdError::UnsupportedCmdStructure
                    )
                };

                let second = parse_u64_arg("COUNT", &mut values)?;
                count = Some(second);
                has_count = true;
            } else if first == KW_BLOCK {
                if has_block {
                    return Err(
                        CmdError::UnsupportedCmdStructure
                    )
                };

                let second = parse_u64_arg("BLOCK", &mut values)?;
                block_ms = Some(second);
                has_block = true;
            } else if first != KW_STREAMS {
                return Err(
                    CmdError::UnsupportedCmdStructure
                )
            } else {
                break;
            };
        };
        
        // Parse STREAMS key1 key2 .. id1 id2
        if values.len() & 1 == 1 {
            // Nbr of key id must be even
            return Err(
                CmdError::UnsupportedCmdStructure
            )
        };

        let mut keys: VecDeque<String> = VecDeque::new();
        let mut ids: VecDeque<(u64, u64)> = VecDeque::new();
        let pair_len = values.len()/2;
        
        // let keys: VecDeque<String> = values.drain(..pair_len).collect();
        // let mut keyskeys.extend(values.drain(..pair_len));
        values.drain(..pair_len).for_each(|t| {
            keys.push_back(
                t.get_value().unwrap().str().unwrap()
                )
        });
        
        values.drain(..pair_len).try_for_each(|t| {
            let (t, i) = Self::_parse_stream_id_xread(
                t.get_value().unwrap().str().unwrap())?;
            ids.push_back((t, i));
            Ok(())
        })?;
        
        // Pairing key and value
        for _ in 0..pair_len {
            let id = ids.pop_front().unwrap(); 
            stream.push((
                keys.pop_front().unwrap(),
                id.0,
                id.1
            ));
        };

        Ok(Self::XREAD { count, block_ms, stream })
    }
    
    pub fn incr(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let key: String = values.pop_front()
           .ok_or(CmdError::MissingArgument("No key provided for INCR".to_string()))?
           .get_value().unwrap().str().unwrap();
        Ok(Self::INCR(key))
    }

    pub fn multi() -> Result<Self, CmdError> {
        Ok(Self::MULTI)
    }

    pub fn exec() -> Result<Self, CmdError> {
        Ok(Self::EXEC)
    }

    pub fn discard() -> Result<Self, CmdError> {
        Ok(Self::DISCARD)
    }

    pub fn watch(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let mut keys: Vec<String> = Vec::new();
        values.drain(..).for_each(|v| keys.push(v.get_value().unwrap().str().unwrap()));
        if keys.is_empty() {
            Err(CmdError::MissingArgument("No key provided for WATCH".to_string()))
        } else {
            Ok(Self::WATCH(keys))
        }
    }

    pub fn unwatch() -> Result<Self, CmdError> {
        Ok(Self::UNWATCH)
    }

    pub fn info(mut values: VecDeque<RespType>) -> Result<Self, CmdError> {
        let key: String = values.pop_front()
           .ok_or(CmdError::MissingArgument("No key provided for INFO".to_string()))?
           .get_value().unwrap().str().unwrap();
        Ok(Self::INFO(key))
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
                                        s if s == KW_PING => {
                                            return Self::ping();
                                        },
                                        s if s == KW_ECHO => {
                                            return Self::echo(v);
                                        },
                                        s if s == KW_SET => {
                                            return Self::set(v);
                                        },
                                        s if s == KW_GET => {
                                            return Self::get(v);
                                        },
                                        s if s == KW_RPUSH => {
                                            return Self::rpush(v);
                                        },
                                        s if s == KW_LRANGE => {
                                            return Self::lrange(v);
                                        },
                                        s if s == KW_LPUSH => {
                                            return Self::lpush(v);
                                        },
                                        s if s == KW_LLEN => {
                                            return Self::llen(v);
                                        },
                                        s if s == KW_LPOP => {
                                            return Self::lpop(v);
                                        },
                                        s if s == KW_BLPOP => {
                                            return Self::blpop(v)
                                        },
                                        s if s == KW_TYPE => {
                                            return Self::ktype(v)
                                        },
                                        s if s == KW_XADD => {
                                            return Self::xadd(v)
                                        },
                                        s if s == KW_XRANGE => {
                                            return Self::xrange(v)
                                        },
                                        s if s == KW_XREAD => {
                                            return Self::xread(v)
                                        },
                                        s if s == KW_INCR => {
                                            return Self::incr(v)
                                        },
                                        s if s == KW_MULTI => {
                                            return Self::multi()
                                        },
                                        s if s == KW_EXEC => {
                                            return Self::exec()
                                        },
                                        s if s == KW_DISCARD => {
                                            return Self::discard()
                                        },
                                        s if s == KW_WATCH => {
                                            return Self::watch(v)
                                        },
                                        s if s == KW_UNWATCH => {
                                            return Self::unwatch()
                                        },
                                        s if s == KW_INFO => {
                                            return Self::info(v)
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
