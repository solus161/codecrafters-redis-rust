use std::cell::RefCell;
use std::collections::VecDeque;
use std::error;
use std::io::{self,  Read, Write};
use std::net::TcpStream;
use std::rc::Rc;
use std::str::from_utf8;

use crate::cmd_handler::CmdHandler;
use crate::resp::RespType;

// #[derive(Debug)]
// pub enum ParsingStatus {
//     ArgsCount(String),
//     ArgLen(String),
//     Arg{ arg: String, len: usize },
//     Delimiter(Box<ParsingStatus>),
//     None,
// }

//What to do next
#[derive(Debug)]
pub enum ParsingStatus {
    ArgsCount(String),
    ArgLen(String),
    Integer,
    ReadBytes(usize),
    Arg{ arg: String, len: usize },
    NextDelimiter,
    Delimiter(Option<Box<ParsingStatus>>),
    None,
}

#[derive(Debug)]
pub enum ArgCompleted {
    Completed(String),
    Incompleted(String)
}

#[derive(Debug)]
pub struct TcpClient {
    pub stream: TcpStream,
    pub buf: VecDeque<u8>,
    pub buf_index: usize,
    pub args: Vec<RespType>,
    pub current_arg: Option<RespType>,
    pub args_count: usize, // total args count in RESP header
    pub parsing_status: ParsingStatus,
    pub cmd_handler: Rc<RefCell<CmdHandler>>,
}

const DELIMITER: &str = "\r\n";
pub const BUFFER_SIZE: i32 = 4096;


impl TcpClient {
    pub fn new(stream: TcpStream, cmd_handler: Rc<RefCell<CmdHandler>>) -> Self {
        Self {
            stream,
            buf: VecDeque::new(),
            buf_index: 0,
            args: Vec::new(), // splitted stream based on \r\n
            current_arg: None,
            args_count: 0,
            parsing_status: ParsingStatus::None,
            cmd_handler,
        }
    }

    pub fn read_socket(&mut self) -> Result<(), Box<dyn error::Error>> {
        let mut tmp_buf = [0u8; BUFFER_SIZE as usize];
        
        // This triggered by epoll_wait and having key matched
        // so there should be data to read
        let n = match self.stream.read(&mut tmp_buf) {
            Ok(0) => return Err("Client disconnected".into()),
            Ok(n) => n,
            Err(e) => return Err(e.into()),
        };
        println!("Socket buf read: {:?}", from_utf8(&tmp_buf[..n]).unwrap());

        // Push tmp_buf into current buf
        //println!("Current buf before append: {:?}", &self.buf);
        self.buf.append(&mut VecDeque::from(tmp_buf[..n].to_vec()));
        //println!("Current buf after append: {:?}", &self.buf);
        
        self.parse_stream()
    }

    fn drain_to_index(&mut self) -> Result<String, io::Error>{
        let output: Vec<u8> = self.buf.drain(..self.buf_index).collect();
        self.buf_index = 0;
        match from_utf8(&output) {
            Ok(s) => Ok(s.to_string()),
            Err(_) => Err(
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Error converting from bytes to utf8"
                    )
                )
        }
    }

    fn parse_stream(&mut self) -> Result<(), Box<dyn error::Error>> {
        // Parsing is now easier as working with Python list
        // Parse all the data of 128 bytes
        
        loop {
            if self.buf.is_empty() {
                return Ok(())
            };
            
            println!("Buf {:?}", &self.buf);
            
            // Cannot move out of struct so have to replace with None
            let parsing_status = std::mem::replace(
                &mut self.parsing_status, ParsingStatus::None);
            match parsing_status {
                // Not parsing anything
                ParsingStatus::None => {
                    // Consume until "*"
                    // while self.buf.len() > 0 {
                    //     if self.buf[0] == b'*'{
                    //         println!("Detect *");
                    //         self.parsing_status = ParsingStatus::ArgsCount(String::new());
                    //         break;
                    //     } else {
                    //         self.buf.pop_front().unwrap();
                    //     }
                    // };
                    
                    while !self.buf.is_empty() {
                        match RespType::match_prefix(self.buf[0]) {
                            Ok(resp_type) => {
                                match resp_type {
                                    RespType::Array(_) => {
                                        self.current_arg = Some(resp_type);
                                        self.parsing_status = ParsingStatus::NextDelimiter;
                                        return Ok(())
                                    }, 
                                    RespType::BulkStr(s) => {
                                        return Ok(())
                                    }
                                    _ => {
                                        self.buf.pop_front().unwrap();
                                        return Ok(())
                                    }
                                } 
                                
                            },
                            Err(e) => {
                                // TODO: implement error handling here 
                                return Ok(()) 
                            }
                        }
                    }
                },
                ParsingStatus::Integer => {

                },
                ParsingStatus::Delimiter(boxed) => {
                        // self.parsing_status = ParsingStatus::Delimiter(boxed);
                    if self.buf.len() >= 2 {
                        // Consume delimiters
                        let slice: Vec<u8> = self.buf.iter().take(2).copied().collect();
                        match from_utf8(&slice) {
                            Ok(delimiter) => {
                                // println!("Trying to parse delimiter {:?}", &delimiter.as_bytes());
                                if delimiter == DELIMITER {
                                    self.buf.drain(..2);
                                } else {
                                    return Err(Box::new(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        format!("Error parsing delimiter \r\n, got {}", &delimiter)
                                    )))
                                }
                            },
                            Err(e) => {
                                return Err(Box::new(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    format!("Error parsing delimiter \r\n: {}", e)
                                )))
                            },
                        }
                    } else {
                        self.parsing_status = ParsingStatus::Delimiter(boxed);
                        return Ok(());
                    };

                    // The delimiter must followed by st else, st valid
                    match *boxed {
                        // The delimiter could followed by another delimiter
                        ParsingStatus::ArgLen(_) => {
                            self.parsing_status = ParsingStatus::ArgLen(String::new());
                            //self.buf.pop_front();
                        },
                        ParsingStatus::Arg{ arg, len } => {
                           self.parsing_status = ParsingStatus::Arg{ arg, len };
                        },
                        _ => {
                            return Err(Box::new(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "Error parsing delimiter"
                            )))
                        }
                    };
                },
                ParsingStatus::ArgsCount(arg) => {
                    println!("ArgsCount: {}", &arg);
                    match self.parse_digit(arg, '*') {
                        Ok(arg) => {
                            match arg {
                                ArgCompleted::Incompleted(arg) => {
                                    self.parsing_status = ParsingStatus::ArgsCount(arg);
                                    return Ok(());
                                },
                                ArgCompleted::Completed(arg) => {
                                    self.args_count = arg.parse::<usize>()?;
                                    self.parsing_status = ParsingStatus::Delimiter(
                                        Box::new(
                                            ParsingStatus::ArgLen(String::new())
                                        )
                                    );
                                },
                            }
                        },
                        Err(e) => {
                            return Err(Box::new(e));
                        }
                    }
                },
                ParsingStatus::ArgLen(arg) => {
                    println!("ArgLen: {}", arg);
                    match self.parse_digit(arg, '$') {
                        Ok(arg) => {
                            match arg {
                                ArgCompleted::Incompleted(arg) => {
                                    self.parsing_status = ParsingStatus::ArgLen(arg);
                                    return Ok(());
                                },
                                ArgCompleted::Completed(arg) => {
                                    self.parsing_status = ParsingStatus::Delimiter(
                                        Box::new(ParsingStatus::Arg{
                                            arg: String::new(),
                                            len: arg.parse::<usize>()?,
                                        })
                                    )
                                }
                            }
                        },
                        Err(e) => {
                            return Err(Box::new(e));
                        },
                    }
                },
                ParsingStatus::Arg{ arg, len } => {
                    println!("Arg {} {}", arg, len);
                    match self.parse_arg(arg, len) {
                        Ok(arg) => {
                            match arg {
                                ArgCompleted::Incompleted(arg) => {
                                    println!("Incompleted Arg {}", arg);
                                    self.parsing_status = ParsingStatus::Arg{ arg, len };
                                    return Ok(());
                                },
                                ArgCompleted::Completed(arg) => {
                                    println!("Completed Arg {}", arg);
                                    self.args.push(arg);
                                    println!("Args {:?}", &self.args);
                                    if self.args.len() == self.args_count {
                                        // All args parsed
                                        let _ = self.response();
                                        self.args_count = 0;
                                        self.parsing_status = ParsingStatus::None;
                                    } else {
                                        self.parsing_status = ParsingStatus::Delimiter(
                                            Box::new(ParsingStatus::ArgLen(String::new()))
                                        );
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            return Err(Box::new(e));
                        }
                    }
                },
            }
        }
    }

    fn response(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Response based on args
        let args = std::mem::replace(&mut self.args, Vec::<String>::new());
        match self.cmd_handler.borrow_mut().handle(args) {
            Some(output_str) => {
                println!("Stream write: {}", &output_str);
                let _ = self.stream.write_all(output_str.as_bytes());
                Ok(())
            },
            None => Ok(()),
        }
    }

    fn next_delimiter(&mut self) {
        // Advance buffer index till next \r
        while !self.buf.is_empty() && self.buf[self.buf_index] != b'\r' {
            self.buf_index += 1;
            if self.buf_index > self.buf.len() {
                break;
            }
        }
    }

    fn parse_digit(&mut self, mut arg: String, prefix: char) -> Result<ArgCompleted, io::Error> {
        // Digit must start with $, else error
        // and end with \r\n
        //println!("Buf before parsing digit {:?}", &self.buf);
        if self.buf.is_empty() {
            Ok(ArgCompleted::Incompleted(arg))
        } else {
            if arg.is_empty() {
                if self.buf[0] != prefix as u8 {
                    return Err(
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "Error parsing digit, starting char must be {}, got {}", 
                                prefix as char, self.buf[0] as char)));
                } else {
                    arg.push(self.buf.pop_front().unwrap() as char);
                };
            };

            while self.buf.len() > 0  {
                if self.buf[0].is_ascii_digit() {
                    arg.push(self.buf.pop_front().unwrap() as char);
                } else {
                    arg.drain(..1);
                    return Ok(ArgCompleted::Completed(arg));
                }
            };
            Ok(ArgCompleted::Incompleted(arg))
        }
    }

    fn parse_arg(&mut self, mut arg: String, len: usize) -> Result<ArgCompleted, io::Error> {
        //println!("Buf {:?}", &self.buf);
        if self.buf.is_empty() {
            Ok(ArgCompleted::Incompleted(arg))
        } else {
            let remaining_len = (len - arg.len()).min(self.buf.len());
            println!("Remaining len {}", &remaining_len);
            let remaining_bytes = self.buf.drain(..remaining_len).collect::<Vec<u8>>();
            println!("Arg before push_str {}", &arg);
            arg.push_str(from_utf8(&remaining_bytes).unwrap().trim_end_matches('\0'));
            println!("Arg after push_str {}", &arg);
            if arg.len() == len {
                Ok(ArgCompleted::Completed(arg))
            } else {
                Ok(ArgCompleted::Incompleted(arg))
            }
        }
    }
}
