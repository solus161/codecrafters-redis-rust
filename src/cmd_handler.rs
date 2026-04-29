use std::collections::{HashMap, VecDeque};
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



//-------Command, struct and parser
#[derive(Debug)]
pub enum Cmd {
    PING, 
    ECHO(String),
    SET { key: String, value: String, opt: Option<CmdOption>  },
    GET { key: String },
}

impl Cmd {
    fn ping() -> Option<Self> { Some(Self::PING) }
    fn echo(mut values: VecDeque<RespType>) -> Option<Self> {
        let s: String = values.pop_front()?.get_value()?.str()?;
        return Some(Self::ECHO(s));
    }
    
    fn set(mut values: VecDeque<RespType> ) -> Option<Self> {
        let key: String = values.pop_front()?.get_value()?.str()?; 
        let value: String = values.pop_front()?.get_value()?.str()?;
        match values.pop_front() {
            // Having option
            Some(o) => {
                let expire_key: String = o.get_value()?.str()?;        
                let expire_value: u64 = match values.pop_front() {
                    Some(o) => {
                        // TODO: handle conversion error
                        o.get_value()?.str()?.parse::<u64>().ok()?
                    },
                    None => return None
                };
                let opt = CmdOption::set(expire_key, expire_value)?;
                Some(Self::SET{ key: key, value: value, opt: Some(opt) })
            },
            // Have no option
            None => Some(Self::SET{ key: key, value: value, opt: None })
        }
    }

    fn get(mut values: VecDeque<RespType>) -> Option<Self>{
        let key: String = values.pop_front()?.get_value()?.str()?;
        Some(Cmd::GET{ key })
    }

    pub fn from_resp(resp_type: RespType) -> Option<Self> {
        // Instantiate Cmd from RespType
        match resp_type {
            RespType::Array{ length, value } => {
                // Iterate through the array to construct Cmd
                // A command is always in array form
                if length == 0 { return None };

                // First item must be cmd type
                if  let Some(mut v) = value {
                    match v.pop_front() {
                        Some(o) => {
                            println!("Cmd: {:?}", &o);
                            match o {
                                RespType::BulkStr { length, value } => {
                                    if length == 0 { return None };

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
                                        }
                                        _ => return None
                                    } 
                                }
                                _ => return None
                            }            
                        },
                        None => return None
                    };
                };

                return None;
            },
            _ => return None,
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
            _ => None  
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
    data: HashMap<String, HashItem>
}

impl CmdHandler {
    pub fn new() -> Self{
        Self { data: HashMap::new() }
    }

    pub fn handle(&mut self, cmd: Cmd) -> Option<String> {
        match cmd {
            Cmd::PING => Self::cmd_ping(),
            Cmd::ECHO(s) => Self::cmd_echo(s),
            Cmd::SET{ key, value, opt } => self.cmd_set(key, value, opt),
            Cmd::GET{ key } => self.cmd_get(key),
            _ => None
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

            self.data.insert(key, HashItem { value: value, expired_at: exp });
        } else {
            self.data.insert(key, HashItem { value: value, expired_at: None });
        }
        Self::response_ok()
    }

    fn cmd_get(&self, key: String) -> Option<String> {
        match self.data.get(&key) {
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
}
