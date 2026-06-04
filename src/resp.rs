#![allow(dead_code)]

use std::io::{ self, Write };
use std::collections::VecDeque;
use std::str;

use crate::exceptions::CustomError;

#[derive(Debug)]
pub enum ParseStatus {
    None,           // Currently parsing nothing, wait for a type prefix
    Type,           // Waiting for type
    Header,         // Applicable for type that has Header
    Line,           // Read till next \r\n
    Bulk(usize),    // Bulk read 
    RDB(usize),     // Parsing RDB
}

pub enum ReadBytesOutput {
    // Either String for BulkStr or Vec<u8> for RDB parsing
    String(String),
    Bytes(Vec<u8>),
}

impl ReadBytesOutput {
    pub fn str(self) -> Option<String> {
        match self {
            Self::String(s) => Some(s),
            _ => None
        }
    }
}

//---------Parser
#[derive(Debug)]
pub struct RespParser {
    pub buf: Vec<u8>,
    pub buf_cur: usize,
    pub tmp_start: usize,
    pub stack: Vec<RespType>,           // Hold incompletly parsed type
    pub completed: VecDeque<RespType>,  // Hold completed type
    pub status: ParseStatus,
}

impl RespParser {
    pub fn new() -> Self {
        Self{ 
            buf: Vec::new(),
            buf_cur: 0,
            tmp_start: 0,
            stack: Vec::new(),
            completed: VecDeque::new(),
            status: ParseStatus::Type }
    }

    pub fn feed_buf(&mut self, data: &[u8], n: usize) {
        self.buf.extend(&data[..n]);
        // println!("Parser read to buf: {:?}", &data[..n]);
    }

    pub fn parse(&mut self, rdb: bool) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            // sleep(Duration::new(1, 0));
            // println!("Buf {:?}", str::from_utf8(&self.buf).unwrap());
            // println!("buf_cur {}, tmp_start {}", &self.buf_cur, &self.tmp_start);
            // Check if top stack is is_completed
            self.pop_completed();
            if !self.completed.is_empty() {
                return Ok(())
            }

            if self.buf.is_empty() || self.buf_cur >= self.buf.len() {
                // println!("Parser buf empty");
                return Ok(());
            };
            
            let current_status = std::mem::replace(&mut self.status, ParseStatus::Type);
            
            match current_status {
                ParseStatus::Type => {
                    // scan till new type
                    // if new type, add to top of stack
                    // if type completed, 
                    // - pop off stack, 
                    // - push down to next top opened type
                    // - or processed by handler
                    // println!("Parsing Type");
                    match self.next_till_type(rdb) {
                        Some(resp_type) => {
                            self.stack.push(resp_type);
                            if let Some(top) = self.stack.last() {
                                match top {
                                    RespType::Array {..} | RespType::BulkStr {..} | RespType::RDB(_) => {
                                        // println!("Found array-like type");
                                        self.status = ParseStatus::Header;
                                    },
                                    RespType::SimpleStr(..) | RespType::Integer(..) => {
                                        // println!("Found simple and scalar type");
                                        self.status = ParseStatus::Line;
                                    },
                                    _ => {
                                        // Not implemented yet
                                    }
                                }
                            };
                        },
                        None => {
                            self.status = ParseStatus::Type;
                        },
                    }
                },
                ParseStatus::Header => {
                    // scan till \r\n
                    // save result to object at top of stack
                    // println!("Parsing Header");
                    match self.next_till_new_line() {
                        Ok(Some(s)) => {
                            // This String s will be consumed by top stack resp type
                            // which has length attr
                            self.drain_till_buf();
                            if let Some(top) = self.stack.last_mut() {
                                // println!("{:?}", &s);
                                let length: usize = s.str().unwrap().parse()?;
                                top.set_length(length);

                                match top {
                                    RespType::Array { .. } => {
                                        self.status = ParseStatus::Type;
                                    },
                                    RespType::BulkStr { length, .. } |
                                    RespType::BulkError { length, .. } |
                                    RespType::VerbatimStr { length, .. } => {
                                        self.status = ParseStatus::Bulk(*length);
                                    },
                                    RespType::RDB(_) => {
                                        self.status = ParseStatus::RDB(length);
                                    },
                                    _ => {
                                        self.status = ParseStatus::Type;
                                    }
                                };
                            };
                        },
                        Ok(None) => {
                            // Not enough bytes to assess new line
                            self.status = ParseStatus::Header;
                            return Ok(());
                        },
                        Err(e) => return Err(e),
                    }
                },
                ParseStatus::Line => {
                    // scan till \r\n, work with types with no length attr
                    // println!("Parsing Line");
                    match self.next_till_new_line() {
                        Ok(Some(s)) => {
                            self.drain_till_buf();
                            if let Some(top) = self.stack.last_mut() {
                                // TODO: handle converting to i64
                                top.set_value(s)?;
                                self.status = ParseStatus::Type;
                            }
                        },
                        Ok(None) => {
                            // Not enough line to assess new line
                            self.status = ParseStatus::Line;
                            return Ok(());
                        },
                        Err(e) => return Err(e)
                    }
                },
                ParseStatus::Bulk(n) => {
                    // println!("Parsing Bulk");
                    match self.read_bytes(n, false) {
                        Ok(o) => {
                            match o {
                                Some(s) => {
                                    // println!("Parsing Bulk s");
                                    self.drain_till_buf();
                                    if let Some(top) = self.stack.last_mut() {
                                        top.set_value(s)?;
                                        self.status = ParseStatus::Type;
                                    };
                                },
                                None => {
                                    // println!("Parsing Bulk None");
                                    self.status = ParseStatus::Bulk(n)
                                },
                            }
                        },
                        Err(_) => {}
                    }
                },
                ParseStatus::RDB(n) => {
                    // println!("Parsing RDB");
                    match self.read_bytes(n, true) {
                        Ok(o) => {
                            match o {
                                Some(s) => {
                                    self.drain_till_buf();
                                    if let Some(top) = self.stack.last_mut() {
                                        top.set_value(s)?;
                                        self.status = ParseStatus::Type;
                                    };
                                },
                                None => {
                                    self.status = ParseStatus::RDB(n)
                                },
                            }
                        },
                        Err(_) => {
                            // TODO: handling error
                            // println!("{}", e);
                        }
                    }
                },
                ParseStatus::None => {
                    // Do nothing
                }
            }
        }
    }

    fn pop_completed(&mut self) {
        // Check for completion of stack from top to bottom
        // Push out final type which could be nested
        loop {
            if self.stack.is_empty() {
                break
            } else {
                let mut completed = false;
                if let Some(top) = self.stack.last() {
                    completed = top.is_completed();
                };
                if completed {
                    let completed_type = self.stack.pop().unwrap();
                    // println!("Completed type {:?}", &completed_type);
                    // println!("Stack {:?}", &self.stack);
                    if self.stack.is_empty() {
                        // There is no more than stack of 2 scalar/simple types
                        // println!("Completed pushed to completed stack: {:?}", &completed_type);
                        self.completed.push_back(completed_type);
                    } else {
                        // The next top must be of array type
                        if let Some(top) = self.stack.last_mut() {
                            top.add_item(completed_type);
                        }
                    }
                } else {
                    break
                }
            }
        }
    }

    pub fn get_completed(&mut self) -> Option<RespType> {
        self.completed.pop_front()
    }

    fn next_till_type(&mut self, rdb: bool) -> Option<RespType> {
        // Consume buf till getting a type
        while !self.buf.is_empty() && self.buf_cur < self.buf.len() {
            match RespType::match_prefix(self.buf[self.buf_cur], rdb) {
                Some(resp_type) => {
                    self.buf_cur += 1;
                    self.tmp_start = self.buf_cur;
                    return Some(resp_type);
                },
                None => {
                    self.buf_cur += 1;
                },
            }
        };
        None
    }

    fn next_till_new_line(&mut self) -> 
        Result<Option<ReadBytesOutput>, Box<dyn std::error::Error>> {
        // Consume buf till getting new line \r\n
        // println!("Head 5 buff: {:?}", self.buf.iter().take(5).collect::<Vec<_>>());
        while self.buf.len() - self.buf_cur >= 2 {
            if self.buf[self.buf_cur] == b'\r' && self.buf[self.buf_cur + 1] == b'\n' {
                match str::from_utf8(&self.buf[self.tmp_start..self.buf_cur]) {
                    Ok(s) => {
                        self.buf_cur += 2;
                        self.tmp_start = self.buf_cur;
                        return Ok(Some(ReadBytesOutput::String(s.to_string())));
                    },
                    Err(e) => {
                        // TODO: convert this to CustomError
                        return Err(Box::new(e))
                    }
                }
                // let output: Vec<u8> = self.tmp.drain(..).collect();
                // println!("Line drained {:?}", &output);
                // match str::from_utf8(&output) {
                //     Ok(s) => {
                //         //  Pop delimiter
                //         self.buf.pop_front();
                //         self.buf.pop_front();
                //         return Ok(Some(ReadBytesOutput::String(s.to_string())));
                //     },
                //     Err(e) => { 
                //         // TODO: handle utf8 converting error
                //         return Err(Box::new(e))
                //     },
                // }
            } else {
                // Pop from buf, push to tmp
                // self.tmp.push(self.buf.pop_front().unwrap());
                self.buf_cur += 1
            };
        };
        Ok(None)
    }

    fn read_bytes(&mut self, n: usize, rdb: bool) ->
        Result<Option<ReadBytesOutput>, Box<dyn std::error::Error>> {
        // Logic change: just read to n bytes
        // - if rdb, end
        // - if not, looking for \r\n
        let tmp_len = self.buf_cur - self.tmp_start;
        let buf_len = self.buf.len();
        let mut ending_len = 2;

        if rdb { ending_len = 0 }; 
        let next_len = (n + ending_len).saturating_sub(tmp_len).min(buf_len);

        // println!("buf_cur before next_len {}", self.buf_cur);
        // println!("next_len {}", next_len);
        
        if next_len > 0 {
            self.buf_cur += next_len;
            // self.tmp.append(&mut self.buf.drain(..next_len).collect::<Vec<u8>>());
            // return Ok(None);
        };

        // println!("read_bytes buf_cur {}, tmp_start {}, n {}, ending_len {}", self.buf_cur, self.tmp_start, n, ending_len);
        
        if self.buf_cur <= self.buf.len() && self.buf_cur - self.tmp_start == n + ending_len {
            // tmp must end with \r\n
            if !rdb && self.buf[self.tmp_start + n] == b'\r' && self.buf[self.tmp_start + n + 1] == b'\n' {
                match str::from_utf8(&self.buf[self.tmp_start..self.buf_cur - 2]) {
                    Ok(s) => {
                        return Ok(Some(ReadBytesOutput::String(s.to_string())));
                    },
                    Err(e) => {
                        // TODO: convert to CustomError
                        Err(Box::new(e))
                    }
                }
                // match str::from_utf8(&self.tmp.drain(..n).collect::<Vec<u8>>()) {
                //     Ok(s) => {
                //         self.tmp.drain(..ending_len);
                //         return Ok(Some(ReadBytesOutput::String(s.to_string()))); 
                //     },
                //     Err(e) => {
                //         // Error converting to utf8
                //         return Err(Box::new(e));
                //     }
                // };
            } else if rdb {
                Ok(Some(ReadBytesOutput::Bytes(self.buf[self.tmp_start..self.buf_cur].into()))) 
            } else {
                // Error ending \r\n not found
                return Err(Box::new(
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Bulk reading must end with \r\n, got {}")
                        )
                )
            }

        } else {
           // Buf is empty
           return Ok(None);
        }
    }

    fn drain_till_buf(&mut self) {
        // Drain all bytes before buf_cur
        self.buf.drain(..self.buf_cur);

        // Reset buf_cur to 0
        self.buf_cur = 0;

        // Reset tmp_start to buf_cur
        self.tmp_start = 0;
    }
}



//------ RespType
#[derive(Debug)]
pub enum RespType {
    SimpleStr(Option<String>),
    Error(Option<String>),
    Integer(Option<i64>),
    BulkStr{ length: usize, value: Option<String> },
    NullBulkStr,
    Array{ length: usize, value: Option<VecDeque<RespType>> },
    Null,
    Bool(Option<i64>),
    Double(Option<f64>),
    BigNbr(Option<String>),
    BulkError{ length: usize, value: Option<String> },
    VerbatimStr{ length: usize, value: Option<String> },
    Map,
    Attr,
    Set,
    Push,
    RDB(Option<Vec<u8>>),
    Bytes(Option<Vec<u8>>),
}


const DELIMITER: &str = "\r\n";

// To contain extracted value from RespType
#[derive(Debug, Clone)]
pub enum RespValue {
    Str(String),
    Integer(i64),
}

impl RespValue {
    pub fn str(self) -> Option<String> {
        match self {
            Self::Str(s) => Some(s),
            _ => None
        }
    }

    pub fn int(self) -> Option<i64> {
        match self {
            Self::Integer(x) => Some(x),
            _ => None
        }
    }
}

// Resp data type
impl RespType {
    pub const fn get_prefix(&self) -> &str {
        match self {
            Self::SimpleStr(_) => "+",
            Self::Error(_) => "-",
            Self::Integer(_) => ":",
            Self::BulkStr{ length: _, value: _ } => "$",
            Self::NullBulkStr => "$-1\r\n",
            Self::Array{ length: _, value: _} => "*",
            Self::Null => "_",
            Self::Bool(_) => "#",
            Self::Double(_) => ",",
            Self::BigNbr(_) => "(",
            Self::BulkError{ length: _, value: _ } => "!",
            Self::VerbatimStr{ length: _, value: _ } => "=",
            Self::Map => "%",
            Self::Attr => "|",
            Self::Set => "~",
            Self::Push => ">",
            Self::RDB(_) => "$",
            Self::Bytes(_) => "",
        }
    }

    pub fn match_prefix(first_char: u8, rdb: bool) -> Option<Self> {
        match first_char as char {
            '+' => Some(Self::SimpleStr(None)),
            '-' => Some(Self::Error(None)),
            ':' => Some(Self::Integer(None)),
            '$' => {
                if rdb {
                    Some(Self::RDB(None))
                } else {
                    Some(Self::BulkStr{ length: 0, value: None })
                }
            },
            '*' => Some(Self::Array{ length: 0, value: None }),
            '_' => Some(Self::Null),
            '#' => Some(Self::Bool(None)),
            ',' => Some(Self::Double(None)),
            '!' => Some(Self::BulkError{ length: 0, value: None }),
            '=' => Some(Self::VerbatimStr{ length: 0, value: None }),
            '%' => Some(Self::Map),
            '|' => Some(Self::Attr),
            '~' => Some(Self::Set),
            '>' => Some(Self::Push),
            _ => None,
        } 
    }

    pub fn set_length(&mut self, value: usize) {
        match self {
            Self::Array { length, .. } | Self::BulkStr { length, .. } => {
                *length = value
            },
            _ => { /* Do nothing */}
        }
    }

    pub fn is_completed(&self) -> bool {
        // To check whether a type is completely parsed_value
        match self {
            Self::SimpleStr(o) | Self::Error(o) => o.is_some(),
            Self::Integer(o) => o.is_some(),
            Self::Array{ length, value } => {
                match value {
                    Some(v) => {
                        if v.len() == *length {
                            true
                        } else {
                            false
                        }
                    },
                    None => false
                }
            },
            Self::BulkStr { value, .. } | Self::BulkError { value, .. } |
            Self::VerbatimStr { value, .. } => value.is_some(),
            Self::RDB(o) => o.is_some(),
            _ => return false,
        }
    }

    pub fn add_item(&mut self, item: RespType) {
        match self {
            Self::Array { value, .. } => {
                value.get_or_insert_with(VecDeque::new).push_back(item);
            },
            _ => { /* Do nothing */}
        }
    }

    pub fn set_value(&mut self, parsed_value: ReadBytesOutput) -> 
        Result<(), Box<dyn std::error::Error>> {
        match parsed_value {
            ReadBytesOutput::String(s) => {
                match self {
                    Self::SimpleStr(o) | Self::Error(o) => {
                        o.get_or_insert(s);
                        Ok(())
                    },
                    Self::BulkStr { value, .. } => {
                        value.get_or_insert(s);
                        Ok(())
                    },
                    _ => {
                        // Not implemented yet
                        Err("Not implemented".into())
                    }
                }
            },
            ReadBytesOutput::Bytes(v) => {
                match self {
                    Self::RDB(o) => {
                        o.get_or_insert(v);
                        Ok(())
                    },
                    _ => {
                        Err("Not implemented".into())
                    }
                }
            }
        }
        
    }

    pub fn get_str(self) -> Option<String> {
        match self {
            Self::SimpleStr(o) => o,
            Self::BulkStr { value, .. } => value,
            _ => None
        }
    }

    pub fn get_int(self) -> Option<i64> {
        match self {
            Self::Integer(o) => o,
            _ => None
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let prefix = self.get_prefix().to_string();
        let mut output: Vec<u8> = Vec::new();
        match self {
            Self::Array { length, value } => {
                // Recursively serialize item
                if let Some(v) = value {
                    write!(&mut output,"{}{}{}", &prefix, *length, DELIMITER).unwrap();
                    for item in v.iter() {
                        output.extend(&item.serialize());
                    };
                } else {
                    write!(&mut output, "{}{}{}", &prefix, -1, DELIMITER).unwrap()
                };
            },
            Self::BulkStr { length, value } => {
                match value {
                    Some(v) => {
                        write!(
                            &mut output, 
                            "{}{}{}{}{}", 
                            &prefix, *length, DELIMITER, v,DELIMITER).unwrap();
                    },
                    None => {
                        write!(&mut output, "{}{}{}", &prefix, -1, DELIMITER).unwrap();
                    },
                }
            },
            Self::SimpleStr(o) => {
                if let Some(s) =  o {
                    write!(&mut output, "{}{}{}", &prefix, s, DELIMITER).unwrap();
                }
            },
            Self::Integer(o) => {
                if let Some(x) = o {
                    write!(&mut output, "{}{}{}", &prefix, x, DELIMITER).unwrap()
                }
            },
            Self::RDB(Some(v)) => {
                write!(&mut output, "{}{}{}", &prefix, v.len(), DELIMITER).unwrap();
                output.extend_from_slice(v);
            },
            Self::Error(o) => {
                if let Some(s) =  o {
                    write!(&mut output, "{}{}{}", &prefix, s, DELIMITER).unwrap()
                }
            },
            Self::Bytes(o) => {
                if let Some(b) =  o { output.extend(b) }
            },
            _ => todo!("Serialize not yet implemented")
        };
        output
    }
}

impl From<CustomError> for RespType {
    fn from(e: CustomError) -> Self {
        e.into_resp() 
    }
}
