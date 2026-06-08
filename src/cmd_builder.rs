use std::collections::{VecDeque};
use std::u64;

use crate::exceptions::CustomError;
use crate::resp::{ RespType };


// Command Keyword
pub const KW_PING: &str = "PING";
pub const KW_PONG: &str = "PONG";
pub const KW_OK: &str = "OK";
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
pub const KW_REPLCONF: &str = "REPLCONF";
pub const KW_PSYNC: &str = "PSYNC";
pub const KW_LISTENING_PORT: &str = "listening-port";
pub const KW_CAPA: &str = "capa";
pub const KW_FULLRESYNC: &str = "FULLRESYNC";
pub const KW_GETACK: &str = "GETACK";
pub const KW_ACK: &str = "ACK";
pub const KW_WAIT: &str = "WAIT";
const KW_CONFIG: &str = "CONFIG";
const KW_KEYS: &str = "KEYS";
const KW_SUBSCRIBE: &str = "SUBSCRIBE";
const KW_UNSUBSCRIBE: &str = "UNSUBSCRIBE";
const KW_PSUBSCRIBE: &str = "PSUBSCRIBE";
const KW_PUNSUBSCRIBE: &str = "PUNSUBSCRIBE";
const KW_QUIT: &str = "QUIT";
const KW_PUBLISH: &str = "PUBLISH";
const KW_ZADD: &str = "ZADD";
const KW_ZRANK: &str = "ZRANK";
const KW_ZRANGE: &str = "ZRANGE";
const KW_ZCARD: &str = "ZCARD";
const KW_ZSCORE: &str = "ZSCORE";
const KW_ZREM: &str = "ZREM";
const KW_GEOADD: &str = "GEOADD";
const KW_GEOPOS: &str = "GEOPOS";
const KW_GEODIST: &str = "GEODIST";
const KW_GEOSEARCH: &str = "GEOSEARCH";
const KW_FROMLONLAT: &str = "FROMLONLAT";
const KW_BYRADIUS: &str = "BYRADIUS";
const KW_ACL: &str = "ACL";
const KW_WHOAMI: &str = "WHOAMI";
const KW_GETUSER: &str = "GETUSER";

#[derive(Debug, PartialEq)]
pub enum CmdArg {
    // This is used when a arg placeholder could have different args of same types
    EX(Option<u64>), // expire in x seconds
    PX(Option<u64>), // expire in x miliseconds
    ListeningPort(u16),
    Capa(String),
    GetAck(String),
    Ack(i64),
    Get(String),
    FromLonLat((f64, f64)),
    ByRadius(f64), // unit meter
    WhoAmI(String),
    GetUser(String),
}

impl CmdArg {
    fn set(key: String, value: String) -> Result<Self, CustomError> {
        match key.as_str() {
            KW_EX => {
                let x = value.parse::<u64>()?;
                Ok(Self::EX(Some(x)))
            },
            KW_PX => {
                let x = value.parse::<u64>()?;
                Ok(Self::PX(Some(x)))
            },
            KW_LISTENING_PORT => {
                let x = value.parse::<u16>()?;
                Ok(Self::ListeningPort(x))
            },
            KW_CAPA => {
                Ok(Self::Capa(value))
            },
            KW_GETACK => {
                Ok(Self::GetAck(value))
            },
            KW_ACK => {
                let x = value.parse::<i64>()?;
                Ok(Self::Ack(x))
            },
            KW_GET => {
                Ok(Self::Get(value))
            },
            _ => Err(CustomError::InvalidArgument(format!("Invalid arg for {}", &key))),
        }
    }
}

//-------Command, struct and parser
#[derive(Debug, PartialEq)]
pub enum Cmd {
    PING,
    PONG,
    OK,
    ECHO(String),
    SET { key: String, value: String, opt: Option<CmdArg>  },
    GET { key: String },
    RPUSH { key: String, value: Vec<String> },
    LRANGE{ key: String, start: i64, stop: i64},
    LPUSH { key: String, value: VecDeque<String> },
    LLEN(String),
    LPOP{ key: String, length: Option<usize> },
    BLPOP{ key: String, timeout_ms: Option<u64> },
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
    REPLCONF(CmdArg),
    PSYNC{ id: String, offset: i64},
    FULLRESYNC { id: String, offset: i64},
    RDB(Vec<u8>),
    WAIT { count: u64, timeout_ms: Option<u64> },
    CONFIG(CmdArg),
    KEYS(String),
    SUBSCRIBE(Vec<String>),
    UNSUBSCRIBE(String),
    PSUBSCRIBE,
    PUNSUBSCRIBE,
    QUIT,
    PUBLISH{ key: String, message: String },
    ZADD{ key: String, score: f64, member: String },
    ZRANK{ key: String, member: String },
    ZRANGE{ key: String, start: i64, end: i64 },
    ZCARD(String),
    ZSCORE{ key: String, member: String },
    ZREM{ key: String, member: String},
    GEOADD{ key: String, long: String, lat: String, member: String },
    GEOPOS{ key: String, members: Vec<String> },
    GEODIST{ key: String, members: Vec<String> },
    GEOSEARCH{ key: String, from_arg: CmdArg, by_arg: CmdArg},
    ACL_WHOAMI,
    ACL_GETUSER(String),
}

impl Cmd {
    pub fn get_name(&self) -> String {
        let name = format!("{:?}", self);
        name.split(&['(', ' ', '{']).next().unwrap().to_string()
    }

    pub const fn to_be_broadcast(&self) -> bool {
        match self {
            Self::SET { .. } | Self::LPUSH { .. } | Self::RPUSH { .. } |
            Self::LPOP { .. } | Self::BLPOP { .. } | Self::INCR(_) | 
            Self::XADD { .. } => {
                true 
            },
            _ => {false}
        }
    }

    pub const fn always_response(&self) -> bool {
        match self {
            Self::REPLCONF(CmdArg::GetAck(_)) => {
                true
            },
            _ => false
        }
    }

    pub const fn is_flag_list(&self) -> bool {
        match self {
            Self::LPUSH { .. } | Self::RPUSH { .. } => {
                true
            },
            _ => false
        }
    }

    pub fn is_flag_stream(&self) -> bool {
        match self {
            Self::XADD { .. } => {
                println!("Set flag stream");
                true
            },
            _ => false
        }
    }

    fn ping() -> Result<Self, CustomError> { Ok(Self::PING) }
    fn pong() -> Result<Self, CustomError> { Ok(Self::PONG) }
    fn ok() -> Result<Self, CustomError> { Ok(Self::OK) }
    fn echo(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg = "No argument provided for ECHO";
        let s: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg.to_string()))?;
        return Ok(Self::ECHO(s));
    }
    
    fn set(mut values: VecDeque<RespType> ) -> Result<Self, CustomError> {
        let msg_key = "No key provided for SET";
        let key: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;
        
        let msg_value = "No value provided for SET";
        let value: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_value.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_value.to_string()))?;

        // Parsing expiration arg
        let msg_exp_key = "No arg provided for expiration";
        let msg_exp_value = "No value provided for expiration";
        match values.pop_front() {
            // Having option
            Some(o) => {
                let expire_key: String = o.get_str()
                    .ok_or(CustomError::MissingArgument(msg_exp_key.to_string()))?;        
                let expire_value: String = match values.pop_front() {
                    Some(o) => {
                        // TODO: handle conversion error
                        o.get_str()
                            .ok_or(CustomError::MissingArgument(msg_exp_value.to_string()))? 
                    },
                    None => return Err(CustomError::MissingArgument(msg_exp_value.to_string()))
                };
                let opt = CmdArg::set(expire_key, expire_value)?;
                Ok(Self::SET{ key, value, opt: Some(opt) })
            },
            // Have no option
            None => Ok(Self::SET{ key, value, opt: None })
        }
    }

    fn get(mut values: VecDeque<RespType>) -> Result<Self, CustomError>{
        let msg = "No key provided for GET";
        let key: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg.to_string()))?;
        Ok(Cmd::GET{ key })
    }

    fn rpush(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for RPUSH";
        let key: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;
        
        let msg_value = "No value provided for RPUSH";
        let mut list_values: Vec<String> = Vec::new();
        while !values.is_empty() {
            // Pop from values, extract String, push to list_values
            let v = values.pop_front()
                .ok_or(CustomError::MissingArgument(msg_value.to_string()))?
                .get_str()
                .ok_or(CustomError::MissingArgument(msg_value.to_string()))?;
            list_values.push(v);
        };
        Ok(Cmd::RPUSH { key, value: list_values })
    }
    
    fn lrange(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for LRANGE";
        let key: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;
        
        let msg_start_index = "No start index provided for LRANGE";
        let start: i64 = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_start_index.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_start_index.to_string()))?
            .parse()?;

        let msg_stop_index = "No end index provided for LRANGE";
        let stop: i64 = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_stop_index.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_stop_index.to_string()))?
            .parse()?;
        Ok(Self::LRANGE { key, start, stop })
    }
    
    fn lpush(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for LPUSH";
        let key: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;
        
        let msg_value = "No value provided for LPUSH";
        let mut list_values: VecDeque<String> = VecDeque::new();
        while !values.is_empty() {
            // Pop from values, extract String, push to list_values
            let v = values.pop_front()
                .ok_or(CustomError::MissingArgument(msg_value.to_string()))?
                .get_str()
                .ok_or(CustomError::MissingArgument(msg_value.to_string()))?;
            list_values.push_back(v);
        };
        Ok(Cmd::LPUSH { key, value: list_values })
    }

    fn llen(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg = "No key provided for LLEN";
        let key: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg.to_string()))?;
        Ok(Cmd::LLEN(key))
    }

    fn lpop(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for LPOP";
        let key: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;
        
        let msg_value = "No length provided for LPOP";
        let length: Option<usize> = match values.pop_front() {
            Some(s) => {
                Some(s.get_str()
                    .ok_or(CustomError::MissingArgument(msg_value.to_string()))?
                    .parse()?)
            },
            None => None
        };

        Ok(Cmd::LPOP{ key, length })
    }

    fn blpop(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for BLPOP";
        let key: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;
        
        let msg_timeout = "No timeout provided for BLPOP";
        let timeout_ms: f64 = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_timeout.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_timeout.to_string()))?
            .parse::<f64>()? * 1000.0;
        
        let msg_timeout_invalid = "Error while parsing expiration for BLPOP";
        if timeout_ms < 0.0 {
            Err(CustomError::InvalidArgument(msg_timeout_invalid.to_string()))
        } else if timeout_ms == 0.0 {
            Ok(Self::BLPOP { key, timeout_ms: None })
        } else {
            Ok(Self::BLPOP { key, timeout_ms: Some(timeout_ms as u64) })
        }
    }
    
    fn ktype(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for TYPE";
        let key: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;
        Ok(Self::TYPE(key))
    }

    fn _parse_stream_id_xadd(value: String) -> Result<(Option<u64>, Option<u64>), CustomError> {
        // Parse timestamp id of stream
        let id: (Option<u64>, Option<u64>);
        if value == "*" {
            id = (None, None);
        
        } else {
            let vec_splitted: Vec<&str> = value.split('-').collect();
            let msg_ts = "Error while parsing stream id for XADD";
            match vec_splitted.as_slice() {
                [t, i] => {
                    
                    let ts: Option<u64> = Some(t.parse::<u64>()?);

                    if *i == "*" {
                        id = (ts, None);  
                    } else {
                        let idx: Option<u64> = Some(i.parse::<u64>()?);
                        id = (ts, idx)
                    }
                },
                _ => return Err(CustomError::InvalidArgument(msg_ts.to_string()))
            };
        };
        Ok(id)
    }

    fn xadd(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for XADD";
        let key: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;

        let msg_time_id = "No time id provided for XADD";
        let timestamp_id: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_time_id.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_time_id.to_string()))?;

        // Parse timestamp_id
        let id = Self::_parse_stream_id_xadd(timestamp_id)?; 
        let mut value: Vec<String> = Vec::new();
        for v in values {
            value.push(v.get_str().ok_or(
                    CustomError::MissingArgument("Invalide key-value pair providec".to_string())
            )?);
        };
        Ok(Self::XADD { key, id, value })
    }

    fn _parse_stream_id(value: String, end: bool) -> Result<(u64, u64), CustomError> {
        let vec_splitted: Vec<&str> = value.split('-').collect();
        let id = match vec_splitted.as_slice() {
            [t, i] => {
                let ts: u64 = t.parse::<u64>()?;
                let idx: u64 = i.parse::<u64>()?;
                (ts, idx)
            },
            [t] => {
                let ts: u64 = t.parse::<u64>()?;
                
                if !end {
                    (ts, 0)
                } else {
                    (ts, u64::MAX)
                }
                
            }
            _ => return Err(CustomError::InvalidArgument("Error while parsing stream id".to_string()))
        };
        Ok(id)
    }
    
    fn _parse_stream_id_xrange(value: String, end: bool) -> Result<(u64, u64), CustomError> {
        // Parse timestamp id of stream
        if value == "-" {
            return Ok((0, 0))
        };
        
        if value == "+" {
            return Ok((u64::MAX, u64::MAX))
        };
        
        Self::_parse_stream_id(value, end)
    }

    fn _parse_stream_id_xread(value: String) -> Result<(u64, u64), CustomError> {
        if value == "$" {
            return Ok((u64::MAX, u64::MAX));
        };
        Self::_parse_stream_id(value, false)
    }

    fn xrange(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for XRANGE";
        let key: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;

        let msg_start_index = "No start id provided for XRANGE";
        let start_id: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_start_index.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_start_index.to_string()))?;
    
        let msg_end_index = "No start id provided for XRANGE";
        let end_id: String = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_end_index.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_end_index.to_string()))?;

        // Extract start id
        let start = Self::_parse_stream_id_xrange(start_id, false)?;
        let end = Self::_parse_stream_id_xrange(end_id, true)?;

        Ok(Self::XRANGE { key, start, end })
    }

    fn xread(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let mut count = None;
        let mut stream: Vec<(String, u64, u64)> = Vec::new();
        let mut block_ms: Option<u64> = None;
       
        let mut has_count = false;
        let mut has_block = false;

        let parse_u64_arg = |values: &mut VecDeque<RespType>|
            -> Result<u64, CustomError> {
            let msg = "No argument provided";
            Ok(values.pop_front()
                .ok_or(CustomError::MissingArgument(msg.to_string()))?
                .get_str()
                .ok_or(CustomError::MissingArgument(msg.to_string()))?
                .parse::<u64>()?)
        };
        
        // COUNT or BLOCK could come first
        loop {
            let msg = "No argument provided for XREAD";
            let mut first = values.pop_front()
                .ok_or(CustomError::MissingArgument(msg.to_string()))?
                .get_str()
                .ok_or(CustomError::MissingArgument(msg.to_string()))?;
            let _ = first.make_ascii_uppercase();

            if first == KW_COUNT {
                if has_count {
                    let msg = "Cannot have more than two COUNT";
                    return Err(
                        CustomError::UnsupportedCmdStructure(msg.to_string())
                    )
                };

                let second = parse_u64_arg(&mut values)?;
                count = Some(second);
                has_count = true;
            } else if first == KW_BLOCK {
                if has_block {
                    let msg = "Cannot have more than two BLOCK";
                    return Err(
                        CustomError::UnsupportedCmdStructure(msg.to_string())
                    )
                };

                let second = parse_u64_arg(&mut values)?;
                block_ms = Some(second);
                has_block = true;
            } else if first != KW_STREAMS {
                let msg = "Wrong argument";
                return Err(
                    CustomError::UnsupportedCmdStructure(msg.to_string())
                )
            } else {
                break;
            };
        };
        
        // Parse STREAMS key1 key2 .. id1 id2
        if values.len() & 1 == 1 {
            // Nbr of key id must be even
            let msg = "Not sufficient key-pair values";
            return Err(
                CustomError::UnsupportedCmdStructure(msg.to_string())
            )
        };

        let mut keys: VecDeque<String> = VecDeque::new();
        let mut ids: VecDeque<(u64, u64)> = VecDeque::new();
        let pair_len = values.len()/2;
        
        // let keys: VecDeque<String> = values.drain(..pair_len).collect();
        // let mut keyskeys.extend(values.drain(..pair_len));
        values.drain(..pair_len).try_for_each(|t| -> Result<(), CustomError> {
            let key = t.get_str()
                .ok_or(CustomError::MissingArgument("Key not provided".to_string()))?;
            keys.push_back(key);
            Ok(())
        })?;
        
        values.drain(..pair_len).try_for_each(|t| -> Result<(), CustomError> {
            let value = t.get_str()
                .ok_or(CustomError::MissingArgument("Value not provided".to_string()))?;
            let (t, i) = Self::_parse_stream_id_xread(value)?;
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
    
    pub fn incr(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg = "No key provided for INCR";
        let key: String = values.pop_front()
           .ok_or(CustomError::MissingArgument(msg.to_string()))?
           .get_str()
           .ok_or(CustomError::MissingArgument(msg.to_string()))?;
        Ok(Self::INCR(key))
    }

    pub fn multi() -> Result<Self, CustomError> {
        Ok(Self::MULTI)
    }

    pub fn exec() -> Result<Self, CustomError> {
        Ok(Self::EXEC)
    }

    pub fn discard() -> Result<Self, CustomError> {
        Ok(Self::DISCARD)
    }

    pub fn watch(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key ="Invalid key for WATCH";
        let mut keys: Vec<String> = Vec::new();
        let _ = values.drain(..).try_for_each(|v: RespType| -> Result<(), CustomError> {
            keys.push(
                v.get_str()
                .ok_or(CustomError::MissingArgument(msg_key.to_string()))?);
            Ok(()) 
        });
        if keys.is_empty() {
            let msg = "No key provided for WATCH";
            Err(CustomError::MissingArgument(msg.to_string()))
        } else {
            Ok(Self::WATCH(keys))
        }
    }

    pub fn unwatch() -> Result<Self, CustomError> {
        Ok(Self::UNWATCH)
    }

    pub fn info(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg = "No key provided for INFO";
        let key: String = values.pop_front()
           .ok_or(CustomError::MissingArgument(msg.to_string()))?
           .get_str()
           .ok_or(CustomError::MissingArgument(msg.to_string()))?;
        Ok(Self::INFO(key))
    }

    pub fn replconf(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for REPLCONF";
        let arg: String = values.pop_front()
           .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
           .get_str()
           .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;

        let msg_value = "No value provided for REPLCONF";
        let value: String = values.pop_front()
           .ok_or(CustomError::MissingArgument(msg_value.to_string()))?
           .get_str()
           .ok_or(CustomError::MissingArgument(msg_value.to_string()))?;
        let opt = CmdArg::set(arg, value)?;
        Ok(Self::REPLCONF(opt))
    }

    pub fn psync(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_id = "No id provided for PSYNC";
        let id: String = values.pop_front()
           .ok_or(CustomError::MissingArgument(msg_id.to_string()))?
           .get_str()
           .ok_or(CustomError::MissingArgument(msg_id.to_string()))?;
        
        let msg_offset = "No offset provided for PSYNC";
        let offset: i64 = values.pop_front()
           .ok_or(CustomError::MissingArgument(msg_offset.to_string()))?
           .get_str()
            .ok_or(CustomError::MissingArgument(msg_offset.to_string()))?
           .parse()?;
        
        Ok(Self::PSYNC{ id, offset })
    }

    pub fn fullresync(s: String) -> Result<Self, CustomError> {
        let mut s_iter = s.split(" "); 
        s_iter.next();

        let msg_id = "No id provided for FULLRESYNC";
        let id: String = s_iter.next()
           .ok_or(CustomError::MissingArgument(msg_id.to_string()))?
           .to_string();
        
        let msg_offset = "No offset provided for FULLRESYNC";
        let offset: i64 = s_iter.next()
           .ok_or(CustomError::MissingArgument(msg_offset.to_string()))?
           .parse()?;

        Ok(Self::FULLRESYNC { id, offset })
    }

    pub fn rdb(v: Option<Vec<u8>>) -> Result<Self, CustomError> {
        match v {
            Some(b) => Ok(Self::RDB(b)),
            None => Err(CustomError::UnprocessableError("RBD stream is empty".to_string()))
        }
    } 

    pub fn wait(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_count = "No count provided for WAIT";
        let count: u64 = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_count.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_count.to_string()))?
            .parse()?;

        let msg_timeout = "No timeout provided for WAIT";
        let timeout_ms: u64 = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_timeout.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_timeout.to_string()))?
            .parse::<u64>()?;
        
        if timeout_ms <= 0 {
            Ok(Cmd::WAIT { count, timeout_ms: None })
        } else {
            Ok(Cmd::WAIT { count, timeout_ms: Some(timeout_ms) })
        }
    }

    pub fn config(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_arg = "No arg provided for CONFIG";
        let arg = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_arg.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_arg.to_string()))?;
        
        let msg_value = "No value provided for CONFIG";
        let value = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_value.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_value.to_string()))?;

        let opt = CmdArg::set(arg, value)?;
        Ok(Cmd::CONFIG(opt))
    }

    pub fn keys(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_arg = "No arg provided for KEYS";
        let arg = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_arg.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_arg.to_string()))?;

        Ok(Cmd::KEYS(arg))
    }

    pub fn subscribe(mut values:VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_arg = "No arg provided for KEYS";

        let mut channels = Vec::new();
        let _ = values.drain(..).try_for_each(|r| -> Result<(), CustomError> {
            channels.push(r.get_str()
                .ok_or(CustomError::MissingArgument(msg_arg.to_string()))?);
            Ok(())
        });
        Ok(Self::SUBSCRIBE(channels))
    }

    pub fn unsubscribe(mut values:VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_arg = "No arg provided for UNSUBSCRIBE";
        let key = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_arg.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_arg.to_string()))?;

        Ok(Self::UNSUBSCRIBE(key))
    }

    pub fn psubscribe() -> Result<Self, CustomError> {
        Ok(Self::PSUBSCRIBE)
    }

    pub fn punsubscribe() -> Result<Self, CustomError> {
        Ok(Self::PUNSUBSCRIBE)
    }

    pub fn quit() -> Result<Self, CustomError> {
        Ok(Self::QUIT)
    }

    fn publish(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No channel name provided for PUBLISH";
        let key = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;

        let msg_msg = "No message provided for PUBLISH";
        let message = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_msg.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_msg.to_string()))?;

        Ok(Self::PUBLISH { key, message })
    }

    fn zadd(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for ZADD";
        let key = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;

        let msg_score = "No score provided for ZADD";
        let score: f64 = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_score.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_score.to_string()))?
            .parse::<f64>()?;

        let msg_member = "No member provided for ZADD";
        let member = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_member.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_member.to_string()))?;

        Ok(Cmd::ZADD { key, score, member })
    }

    fn zrank(mut values: VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for ZRANK";
        let key = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;

        let msg_member = "No member provided for ZRANK";
        let member = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_member.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_member.to_string()))?;

        Ok(Cmd::ZRANK { key, member }) 
    }

    fn zrange(mut values:VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for ZRANGE";
        let key = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;

        let msg_start = "No start index provided for ZRANGE";
        let start = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_start.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_start.to_string()))?
            .parse::<i64>()?;

        let msg_end = "No end index provided for ZRANGE";
        let end = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_end.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_end.to_string()))?
            .parse::<i64>()?;

        Ok(Cmd::ZRANGE { key, start, end })
    }

    fn zcard(mut values:VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for ZCARD";
        let key = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;

        Ok(Cmd::ZCARD(key))
    }

    fn zscore(mut values:VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for ZSCORE";
        let key = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;

        let msg_member = "No member provided for ZSCORE";
        let member = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_member.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_member.to_string()))?;

        Ok(Cmd::ZSCORE { key, member })
    }

    fn zrem(mut values:VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for ZREM";
        let key = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;
        
        let msg_member = "No member provided for ZREM";
        let member = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_member.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_member.to_string()))?;

        Ok(Cmd::ZREM{ key, member })
    }

    fn geoadd(mut values:VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for GEOADD";
        let key = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;
        
        let msg_long = "No longitude provided for GEOADD";
        let long = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_long.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_long.to_string()))?
            .parse::<f64>()?
            .to_string();
        
        let msg_lat = "No latitude provided for GEOADD";
        let lat = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_lat.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_lat.to_string()))?
            .parse::<f64>()?
            .to_string();
         
        let msg_member = "No member provided for GEOADD";
        let member = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_member.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_member.to_string()))?;   

        Ok(Cmd::GEOADD { key, long, lat, member })
    }

    fn geopos(mut values:VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for GEOPOS";
        let key = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;

        let mut members: Vec<String> = Vec::new();
        let _ = values.drain(..).try_for_each(|v| -> Result<(), CustomError> {
            members.push(
                v.get_str()
                .ok_or(CustomError::MissingArgument("No member provided".to_string()))?
            );
            Ok(())
        });

        Ok(Cmd::GEOPOS { key, members })
    }

    fn geodist(mut values:VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for GEODIST";
        let key = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;
        
        let msg_members = "Need 2 locations for distance calculation";
        let mut members: Vec<String> = Vec::new();
        let _ = values.drain(..2).try_for_each(|v| -> Result<(), CustomError> {
            members.push(
                v.get_str()
                    .ok_or(CustomError::MissingArgument(msg_members.to_string()))?
            );
            Ok(()) 
        });

        Ok(Self::GEODIST { key, members })
    }

    fn geosearch(mut values:VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_key = "No key provided for GEOSEARCH";
        let key = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_key.to_string()))?;

        let msg_from_arg = "No FROM arg provided";
        let _from_arg_str = values.pop_front() // DEFAULT FROMLONLAT
            .ok_or(CustomError::MissingArgument(msg_from_arg.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_from_arg.to_string()))?;

        let msg_long_lat = "No long/lat provided";
        let long = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_long_lat.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_long_lat.to_string()))?
            .parse::<f64>()?;
        let lat = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_long_lat.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_long_lat.to_string()))?
            .parse::<f64>()?;
        let from_arg = CmdArg::FromLonLat((long, lat));
        
        let msg_by_arg = "No BY arg provided";
        let _by_arg_str = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_by_arg.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_by_arg.to_string()))?;
        
        let msg_radius = "No radius provided";
        let radius: f64 = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_radius.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_radius.to_string()))?
            .parse::<f64>()?;
        let _unit = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_radius.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_radius.to_string()))?;
        let by_arg = CmdArg::ByRadius(radius);
        
        // TODO: this is partly implemented, so no need to check for unit

        Ok(Cmd::GEOSEARCH { key, from_arg, by_arg })
    }

    fn acl(mut values:VecDeque<RespType>) -> Result<Self, CustomError> {
        let msg_kw = "No arg provided for ACL";
        let kw = values.pop_front()
            .ok_or(CustomError::MissingArgument(msg_kw.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument(msg_kw.to_string()))?;

        match kw.as_str() {
            KW_WHOAMI => {
                Ok(Cmd::ACL_WHOAMI)
            },
            KW_GETUSER => {
                let msg = "No username provided";
                let username = values.pop_front()
                    .ok_or(CustomError::MissingArgument(msg.to_string()))?
                    .get_str()
                    .ok_or(CustomError::MissingArgument(msg.to_string()))?;

                Ok(Cmd::ACL_GETUSER(username))
            },
            _ => Err(CustomError::UnsupportedCmdStructure("Unsupported".to_string()))
        }
    }

    pub fn from_resp(resp_type: RespType) -> Result<Self, CustomError> {
        // Instantiate Cmd from RespType
        match resp_type {
            RespType::Array{ length, value } => {
                // Iterate through the array to construct Cmd
                // A command is always in array form
                if length == 0 { return Err(CustomError::NoCmdError("No command".to_string())) };

                // First item must be cmd type
                if let Some(mut v) = value {
                    match v.pop_front() {
                        Some(o) => {
                            match o {
                                RespType::BulkStr { length, value } => {
                                    if length == 0 {
                                        return Err(CustomError::NoCmdError("No command".to_string()))
                                    };

                                    println!("BulkStr value {:?}", &value);

                                    let mut s = value.unwrap();
                                    s.make_ascii_uppercase();
                                    match s.as_str() {
                                        KW_PING => Self::ping(),
                                        KW_ECHO => Self::echo(v),
                                        KW_SET => Self::set(v),
                                        KW_GET =>  Self::get(v),
                                        KW_RPUSH => Self::rpush(v),
                                        KW_LRANGE => Self::lrange(v),
                                        KW_LPUSH => Self::lpush(v),
                                        KW_LLEN => Self::llen(v),
                                        KW_LPOP => Self::lpop(v),
                                        KW_BLPOP => Self::blpop(v),
                                        KW_TYPE => Self::ktype(v),
                                        KW_XADD => Self::xadd(v),
                                        KW_XRANGE => Self::xrange(v),
                                        KW_XREAD => Self::xread(v),
                                        KW_INCR => Self::incr(v),
                                        KW_MULTI => Self::multi(),
                                        KW_EXEC => Self::exec(),
                                        KW_DISCARD => Self::discard(),
                                        KW_WATCH => Self::watch(v),
                                        KW_UNWATCH => Self::unwatch(),
                                        KW_INFO => Self::info(v),
                                        KW_REPLCONF => Self::replconf(v),
                                        KW_PSYNC => Self::psync(v),
                                        KW_WAIT => Self::wait(v),
                                        KW_CONFIG => Self::config(v),
                                        KW_KEYS => Self::keys(v),
                                        KW_SUBSCRIBE => Self::subscribe(v),
                                        KW_UNSUBSCRIBE => Self::unsubscribe(v),
                                        KW_PSUBSCRIBE => Self::psubscribe(),
                                        KW_PUNSUBSCRIBE => Self::punsubscribe(),
                                        KW_QUIT => Self::quit(),
                                        KW_PUBLISH => Self::publish(v),
                                        KW_ZADD => Self::zadd(v),
                                        KW_ZRANK => Self::zrank(v),
                                        KW_ZRANGE => Self::zrange(v),
                                        KW_ZCARD => Self::zcard(v),
                                        KW_ZSCORE => Self::zscore(v),
                                        KW_ZREM => Self::zrem(v),
                                        KW_GEOADD => Self::geoadd(v),
                                        KW_GEOPOS => Self::geopos(v),
                                        KW_GEODIST => Self::geodist(v),
                                        KW_GEOSEARCH => Self::geosearch(v),
                                        KW_ACL => Self::acl(v),
                                        _ => Err(
                                            CustomError::InvalidArgument("Invalid command".to_string()))
                                    } 
                                },
                                _ => Err(CustomError::InvalidArgument("Invalid command".to_string()))
                            }            
                        },
                        None => Err(CustomError::NoCmdError("No command provided".to_string())) 
                    }
                } else {
                    Err(CustomError::NoCmdError("No command provided".to_string()))
                }
            },
            RespType::SimpleStr(o) => {
                let s = o.unwrap();
                let _ = s.to_ascii_uppercase();
                match s.as_str() {
                    KW_PONG => Self::pong(),
                    KW_OK => Self::ok(),
                    p if p[..10] == *KW_FULLRESYNC => {
                            Self::fullresync(s)
                        },
                    _ => Err(
                        CustomError::InvalidArgument("Invalid command".to_string()))
                }
            },
            RespType::RDB(o) => Self::rdb(o),
            _ => return Err(CustomError::UnsupportedCmdStructure("Unsupported structure".to_string())),
        }
    }
}


