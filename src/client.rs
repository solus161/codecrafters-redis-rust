use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::str::from_utf8;

#[derive(Debug)]
pub enum ParsingStatus {
    ArgsCount(String),
    ArgLen(String),
    Arg{ arg: String, len: usize },
    Delimiter(Box<ParsingStatus>),
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
    pub args: Vec<String>,
    pub args_count: usize, // total args count in RESP header
    pub parsing_status: ParsingStatus,
}

const DELIMITER: &str = "\r\n";
pub const BUFFER_SIZE: i32 = 4096;


impl TcpClient {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream,
            buf: VecDeque::new(),
            args: Vec::new(), // splitted stream based on \r\n
            args_count: 0,
            parsing_status: ParsingStatus::None
        }
    }

    pub fn read_socket(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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

    fn parse_stream(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Parsing is now easier as working with Python list
        // Parse all the data of 128 bytes
        
        loop {
            if self.buf.len() == 0 {
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
                    while self.buf.len() > 0 {
                        if self.buf[0] == b'*'{
                            println!("Detect *");
                            self.parsing_status = ParsingStatus::ArgsCount(String::new());
                            break;
                        } else {
                            self.buf.pop_front().unwrap();
                        }
                    }; 
                },
                ParsingStatus::Delimiter(boxed) => {
                    if self.buf.len() < 2 {
                        // Delimiter may not completed in current stream
                        self.parsing_status = ParsingStatus::Delimiter(boxed);
                        return Ok(());
                    } else {
                        let slice: Vec<u8> = self.buf.iter().take(2).copied().collect();
                        match from_utf8(&slice) {
                            Ok(delimiter) => {
                                if delimiter == DELIMITER {
                                    self.buf.drain(..2);
                                }
                            },
                            Err(e) => {
                                panic!{"Error parsing delimiter \r\n: {}", e}
                            },
                        };
                    };
                    
                    // The delimiter must followed by st else, st valid
                    match *boxed {
                         ParsingStatus::ArgLen(_) => {
                             self.parsing_status = ParsingStatus::ArgLen(String::new());
                             //self.buf.pop_front();
                         },
                         ParsingStatus::Arg{ arg, len } => {
                            self.parsing_status = ParsingStatus::Arg{ arg, len };
                         },
                         _ => panic!("Error parsing stream"),
                    };
                },
                ParsingStatus::ArgsCount(arg) => {
                    println!("ArgsCount: {}", &arg);
                    match self.parse_digit(arg, '*') {
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
                ParsingStatus::ArgLen(arg) => {
                    println!("ArgLen: {}", arg);
                    match self.parse_digit(arg, '$') {
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
                ParsingStatus::Arg{ arg, len } => {
                    println!("Arg {} {}", arg, len);
                    match self.parse_arg(arg, len) {
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
                                self.parsing_status = ParsingStatus::None;
                            } else {
                                return Ok(())
                            }
                        }
                    }
                },
            }
        }
    }

    fn response(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Response based on args
        for arg in self.args.drain(..) {
            if arg == "PING" {
                let _ = self.stream.write_all("+PONG\r\n".as_bytes());
            };
        };
        Ok(())
    }

    fn parse_digit(&mut self, mut arg: String, prefix: char) -> ArgCompleted {
        // Digit must start with $, else error
        //println!("Buf before parsing digit {:?}", &self.buf);
        if self.buf.len() == 0 {
            ArgCompleted::Incompleted(arg)
        } else {
            if arg.len() == 0 {
                if self.buf[0] != prefix as u8 {
                    panic!(
                        "Error parsing digit, starting char must be {}, got {}", 
                        prefix as char, self.buf[0] as char);
                } else {
                    arg.push(self.buf.pop_front().unwrap() as char);
                };
            };

            while self.buf.len() > 0  {
                if self.buf[0].is_ascii_digit() {
                    arg.push(self.buf.pop_front().unwrap() as char);
                } else {
                    arg.drain(..1);
                    return ArgCompleted::Completed(arg);
                }
            };
            ArgCompleted::Incompleted(arg)
        }
    }

    fn parse_arg(&mut self, mut arg: String, len: usize) -> ArgCompleted {
        //println!("Buf {:?}", &self.buf);
        if self.buf.len() == 0 {
            ArgCompleted::Incompleted(arg)
        } else {
            let remaining_len = (len - arg.len()).min(self.buf.len());
            println!("Remaining len {}", &remaining_len);
            let remaining_bytes = self.buf.drain(..remaining_len).collect::<Vec<u8>>();
            println!("Arg before push_str {}", &arg);
            arg.push_str(from_utf8(&remaining_bytes).unwrap().trim_end_matches('\0'));
            println!("Arg after push_str {}", &arg);
            if arg.len() == len {
                ArgCompleted::Completed(arg)
            } else {
                ArgCompleted::Incompleted(arg)
            }
        }
    }
}
