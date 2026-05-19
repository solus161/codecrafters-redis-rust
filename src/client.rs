use std::cell::RefCell;
use std::collections::VecDeque;
use std::error;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::rc::Rc;
use std::str::from_utf8;

use crate::config::{self, Config, Replication};
use crate::cmd_handler::CmdHandler; 
use crate::cmd_builder::{Cmd, CmdArg, KW_CAPA, KW_LISTENING_PORT, KW_PING, KW_PSYNC, KW_REPLCONF};
use crate::resp::{ RespType, RespParser };

#[derive(PartialEq)]
enum HandShakeState {
    PING,
    REPLCONF1,
    REPLCONF2,
    PSYNC,
    Established,
    None,
}

struct ReplState {
    pub port: Option<u16>,
    pub capa: Option<String>,
    pub id: String,
    pub offset: i64,
}

pub struct TcpClient {
    pub fd_key: u64,
    pub stream: TcpStream,
    pub resp_parser: RespParser,
    pub cmd_handler: Rc<RefCell<CmdHandler>>,
    handshake_state: HandShakeState,
    repl_state: ReplState,
}

pub const BUFFER_SIZE: i32 = 4096;

impl TcpClient {
    pub fn new(
        fd_key: u64,
        stream: TcpStream, 
        cmd_handler: Rc<RefCell<CmdHandler>>) -> Self {
        Self {
            fd_key: fd_key,
            stream: stream,
            resp_parser: RespParser::new(),
            cmd_handler: cmd_handler,
            handshake_state: HandShakeState::None,
            repl_state: ReplState {
                port: None, capa: None, id: "?".to_string(), offset: -1 },
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
        // println!("{:?}", &tmp_buf[..n]);
        // println!("Socket buf read: {:?}", from_utf8(&tmp_buf[..n]).unwrap());

        // Push tmp_buf into current buf
        //println!("Current buf before append: {:?}", &self.buf);
        self.resp_parser.feed_buf(&tmp_buf, n);
        //println!("Current buf after append: {:?}", &self.buf);
        
        // Parse stream
        self.resp_parser.parse()?;
        
        // Proccess command
        loop {
            match self.resp_parser.get_completed() {
                Some(t) => {
                    let cmd = Cmd::from_resp(t);
                    println!("Cmd completed: {:?}", &cmd);
                    let mut response: Option<String> = None;
                    
                    match cmd {
                        Ok(c) => {
                            // Establishing handshake
                            match self.handshake_state {
                                // Client establishing handshake
                                HandShakeState::PING if matches!(c, Cmd::PONG) => {
                                    self.handshake_state = HandShakeState::REPLCONF1;
                                    response = self.get_replconf1();
                                },
                                HandShakeState::REPLCONF1 if matches!(c, Cmd::OK) => {
                                    self.handshake_state = HandShakeState::REPLCONF2;
                                    response = self.get_replconf2();
                                },
                                HandShakeState::REPLCONF2 if matches!(c, Cmd::OK) => {
                                    self.handshake_state = HandShakeState::PSYNC;
                                    response = self.get_psync();
                                },
                                HandShakeState::REPLCONF2 if matches!(c, Cmd::OK) => {
                                    self.handshake_state = HandShakeState::REPLCONF2;
                                    response = self.get_replconf2();
                                },
                                HandShakeState::PSYNC => {
                                    self.handshake_state = HandShakeState::Established
                                },
                                HandShakeState::Established => {},
                                // Master received handshake request
                                HandShakeState::None  => {
                                    match c {
                                        Cmd::REPLCONF(CmdArg::ListeningPort(x)) => {
                                            self.repl_state.port = Some(x);
                                            response = CmdHandler::response_ok().unwrap().serialize();
                                        },
                                        Cmd::REPLCONF(CmdArg::Capa(s)) => {
                                            self.repl_state.capa = Some(s);
                                            response = CmdHandler::response_ok().unwrap().serialize();
                                        },
                                        _ => {
                                            // Neither a slave not a master
                                            response = self.cmd_handler.borrow_mut().handle(Ok(c), self.fd_key)
                                        }
                                    }
                                }
                                _ => {}
                            };
                        },
                        Err(_) => {}
                    };
                    
                    if let Some(r) = response { 
                        self.stream.write_all(r.as_bytes())?;
                    }
                },
                None => break,
            }
        };
        Ok(())
    }

    pub fn init_handshake(&mut self) {
        self.handshake_state = HandShakeState::PING;
        let mut arr = RespType::Array { length: 1, value: None};
        let ping = RespType::BulkStr { length: KW_PING.len(), value: Some(KW_PING.to_string()) };
        arr.add_item(ping);
        let _ = self.stream.write_all(arr.serialize().unwrap().as_bytes());
    }

    fn get_replconf1(&self) -> Option<String> {
        // Return RESP bytes representing: REPLCONF listening-port <PORT>
        let mut arr = RespType::Array { length: 3, value: None }; 
        let resp_replconf = RespType::BulkStr { length: KW_REPLCONF.len(), value: Some(KW_REPLCONF.to_string()) };
        let resp_listening = RespType::BulkStr { length: KW_LISTENING_PORT.len(), value: Some(KW_LISTENING_PORT.to_string()) };
        let port: String = Config::get().port.to_string();
        let resp_port = RespType::BulkStr { length: port.len(), value: Some(port) };
        arr.add_item(resp_replconf);
        arr.add_item(resp_listening);
        arr.add_item(resp_port);
        arr.serialize()
    }

    fn get_replconf2(&self) -> Option<String> {
        let mut arr = RespType::Array { length: 3, value: None }; 
        let resp_replconf = RespType::BulkStr { length: KW_REPLCONF.len(), value: Some(KW_REPLCONF.to_string()) };
        let resp_capa = RespType::BulkStr { length: KW_CAPA.len(), value: Some(KW_CAPA.to_string()) };
        let psync2 = "psync2";
        let resp_psync = RespType::BulkStr { length: psync2.len(), value: Some(psync2.to_string()) };
        arr.add_item(resp_replconf);
        arr.add_item(resp_capa);
        arr.add_item(resp_psync);
        arr.serialize()
    }

    fn get_psync(&self) -> Option<String> {
        let mut arr = RespType::Array { length: 3, value: None }; 
        let resp_psync = RespType::BulkStr { length: KW_PSYNC.len(), value: Some(KW_PSYNC.to_string()) };
        let resp_id = RespType::BulkStr { 
            length: self.repl_state.id.len(),
            value: Some(self.repl_state.id.clone()) };
        let offset = self.repl_state.offset.to_string();
        let resp_offset = RespType::BulkStr { 
            length: offset.len(),
            value: Some(offset) };
        arr.add_item(resp_psync);
        arr.add_item(resp_id);
        arr.add_item(resp_offset);
        arr.serialize()
    }
}
