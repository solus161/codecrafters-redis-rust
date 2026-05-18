use std::cell::RefCell;
use std::collections::VecDeque;
use std::error;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::rc::Rc;
use std::str::from_utf8;

use crate::config::{self, Config, Replication};
use crate::cmd_handler::CmdHandler; 
use crate::cmd_builder::{Cmd, KW_CAPA, KW_LISTENING_PORT, KW_PING, KW_PSYNC, KW_REPLCONF};
use crate::resp::{ RespType, RespParser };

pub enum ClientRole {
    Master,
    Slave,
    None
}

#[derive(PartialEq)]
enum HandShakeState {
    PING,
    REPLCONF1,
    REPLCONF2,
    PSYNC,
    Established,
    None,
}

struct HandShake {
    request_seq: Vec<String>,
    response_seq: VecDeque<String>,
}

pub struct TcpClient {
    pub role: ClientRole,
    handshake_state: HandShakeState,
    pub fd_key: u64,
    pub stream: TcpStream,
    pub resp_parser: RespParser,
    pub cmd_handler: Rc<RefCell<CmdHandler>>,
}

pub const BUFFER_SIZE: i32 = 4096;

impl TcpClient {
    pub fn new(
        role: Option<ClientRole>,
        fd_key: u64,
        stream: TcpStream, 
        cmd_handler: Rc<RefCell<CmdHandler>>) -> Self {
        Self {
            role: role.unwrap_or(ClientRole::None),
            handshake_state: HandShakeState::None,
            fd_key: fd_key,
            stream: stream,
            resp_parser: RespParser::new(),
            cmd_handler: cmd_handler,
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
                                HandShakeState::PING if c == Cmd::PONG => {
                                    self.handshake_state = HandShakeState::REPLCONF1;
                                    response = self.get_replconf1();
                                },
                                HandShakeState::REPLCONF1 if c == Cmd::OK => {
                                    self.handshake_state = HandShakeState::REPLCONF2;
                                    response = self.get_replconf2();
                                },
                                HandShakeState::REPLCONF2 if c == Cmd::OK => {
                                    self.handshake_state = HandShakeState::PSYNC;
                                    response = self.get_psync();
                                },
                                HandShakeState::REPLCONF2 if c == Cmd::OK => {
                                    self.handshake_state = HandShakeState::REPLCONF2;
                                    response = self.get_replconf2();
                                },
                                HandShakeState::PSYNC => {
                                    self.handshake_state = HandShakeState::Established
                                },
                                HandShakeState::Established => {},
                                HandShakeState::None => {
                                    // Not a slave node
                                    response = self.cmd_handler.borrow_mut().handle(Ok(c), self.fd_key)
                                },
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
        let replconf = RespType::BulkStr { length: KW_REPLCONF.len(), value: Some(KW_REPLCONF.to_string()) };
        let resp_capa = RespType::BulkStr { length: KW_CAPA.len(), value: Some(KW_CAPA.to_string()) };
        let psync2 = "psync2";
        let resp_psync = RespType::BulkStr { length: psync2.len(), value: Some(psync2.to_string()) };
        arr.add_item(replconf);
        arr.add_item(resp_capa);
        arr.add_item(resp_psync);
        arr.serialize()
    }

    fn get_psync(&self) -> Option<String> {
        let mut arr = RespType::Array { length: 1, value: None }; 
        let replconf = RespType::BulkStr { length: KW_PSYNC.len(), value: Some(KW_REPLCONF.to_string()) };
        arr.add_item(replconf);
        arr.serialize()
    }
}
