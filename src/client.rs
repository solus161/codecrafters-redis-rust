use std::cell::RefCell;
use std::collections::VecDeque;
use std::error;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::rc::Rc;
use std::str::from_utf8;

use base64::write;

use crate::exceptions::{CustomError, ERR_HOST_STATS_NOT_INITIATED, ERR_MASTER_STATS_PORT_NOT_SET};
use crate::{ AppStates, ReplStats };
use crate::cmd_handler::CmdHandler; 
use crate::cmd_builder::{
    Cmd, CmdArg, CmdError, KW_CAPA, KW_FULLRESYNC, KW_LISTENING_PORT, KW_OK, KW_PING, KW_PSYNC, KW_REPLCONF};
use crate::resp::{ RespType, RespParser };
use crate::ClientTable;

#[derive(PartialEq)]
enum HandShakeState {
    PING,
    REPLCONF1,
    REPLCONF2,
    PSYNC,
    Established,
    None,
}

enum ReplStatus {
    Master,
    Slave,
}

struct ReplState {
    pub status: Option<ReplStatus>,
    pub port: Option<u16>,
    pub capa: Option<String>,
    pub id: String,
    pub offset: i64,
}

impl ReplState {
    pub fn new() -> Self {
        Self{ status: None, port: None, capa: None, id: "?".to_string(), offset: -1 }
    }
}

pub struct TcpClient {
    pub client_fd: u64,
    pub epoll_fd: i32,          // fd of the epoll queue to control the timer
    pub stream: TcpStream,
    pub resp_parser: RespParser,
    pub cmd_handler: Rc<RefCell<CmdHandler>>,
    handshake_state: HandShakeState,
    // repl_status: ReplStatus,
    repl_state: ReplState,
    parse_rdb: bool,
}

pub const BUFFER_SIZE: i32 = 4096;

impl TcpClient {
    pub fn new(
        client_fd: u64,
        epoll_fd: i32,
        stream: TcpStream, 
        cmd_handler: Rc<RefCell<CmdHandler>>) -> Self {
        Self {
            client_fd: client_fd,
            epoll_fd: epoll_fd,
            stream: stream,
            resp_parser: RespParser::new(),
            cmd_handler: cmd_handler,
            handshake_state: HandShakeState::None,
            // repl_status: ReplStatus::None,
            repl_state: ReplState::new(),
            parse_rdb: false,
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
        // println!("Socket buf read: {:?}", from_utf8(&tmp_buf[..n]).unwrap_or("binary"));

        // Push tmp_buf into current buf
        //println!("Current buf before append: {:?}", &self.buf);
        self.resp_parser.feed_buf(&tmp_buf, n);
        //println!("Current buf after append: {:?}", &self.buf);
        
        // Parse stream
        // self.resp_parser.parse(self.parse_rdb)?;
        
        // Proccess command
        loop {
            // Parse stream
            self.resp_parser.parse(self.parse_rdb)?;
            // 3 cases:
            // - If the client is a slave, the client should not send anything to Master
            // - If the client is a master, all resp recieved must be processed without response
            // - If the client is neither a slave nor a master, all received resp 
            //   must be propagated to other slave, without converting to cmd
            match self.resp_parser.get_completed() {
                Some(t) => {
                    println!("Completed type {:?}", t);
                    let buf = t.to_bytes().unwrap(); // for broadcasting
                    let cmd = Cmd::from_resp(t);
                    let response: Result<Option<Vec<u8>>, Vec<u8>>;

                    match cmd {
                        Ok(c) => {
                            // println!("{:?}", &c);
                            match self.repl_state.status {
                                Some(ReplStatus::Master) => {
                                    response = self.handshake_to_master(c, buf, self.client_fd, self.epoll_fd);
                                },
                                // Some(ReplStatus::Slave) => {
                                //     response = self.handshake_to_client(c, self.client_fd);
                                // }
                                _ => {
                                    response = self.cmd_handler.borrow_mut().handle(
                                        Ok(c), buf,
                                        self.client_fd, self.epoll_fd);
                                }
                            }
                        },
                        Err(e) => {
                            response = self.cmd_handler.borrow_mut()
                                .handle(Err(e), buf, self.client_fd, self.epoll_fd) 
                            // return Err(Box::new(e)),
                        }
                    };
                    
                    match response { 
                        Ok(Some(v)) => {
                            // No error, could broadcasting here
                            self.write_all(&v)?;
                        },
                        Err(v) => {
                            // No broadcasting
                            self.write_all(&v)?;
                        },
                        _ => {}
                        
                    };
                },
                None => break,
            }
        };
        Ok(())
    }

    pub fn write_all(&mut self, buf: &Vec<u8>) -> Result<(), std::io::Error> {
        self.stream.write_all(buf)
    }

    fn handshake_to_master(&mut self, cmd: Cmd, buf: Vec<u8>, client_id: u64, epoll_fd: i32)
        -> Result<Option<Vec<u8>>, Vec<u8>> {
        // Client establishes handshake
        // println!("Cmd from master {:?}", &cmd);
        match self.handshake_state {
            HandShakeState::PING if matches!(cmd, Cmd::PONG) => {
                self.handshake_state = HandShakeState::REPLCONF1;
                self.get_replconf1()
            },
            HandShakeState::REPLCONF1 if matches!(cmd, Cmd::OK) => {
                self.handshake_state = HandShakeState::REPLCONF2;
                self.get_replconf2()
            },
            HandShakeState::REPLCONF2 if matches!(cmd, Cmd::OK) => {
                self.handshake_state = HandShakeState::PSYNC;
                self.get_psync()
            },
            HandShakeState::PSYNC if matches!(cmd, Cmd::FULLRESYNC { .. }) => {
                self.handshake_state = HandShakeState::Established;
                let _ =ClientTable::get().borrow_mut().set_master(client_id);
                self.parse_rdb = true;
                Ok(None)
            },
            HandShakeState::Established if matches!(cmd, Cmd::RDB(_)) => {
                // Do st with the RDB
                self.parse_rdb = false;
                
                // After this, server starts counting for bytes from master
                AppStates::get().get_host_stats()
                    .ok_or_else(||
                        CustomError::InternalError(ERR_HOST_STATS_NOT_INITIATED.to_string()))?
                        .start_bytes_count();
                Ok(None)
            },
            _ => {
                // TODO
                self.cmd_handler.borrow_mut().handle(Ok(cmd), buf, client_id, epoll_fd)
            }
        }
    }
    
    pub fn init_handshake(&mut self) {
        self.repl_state.status = Some(ReplStatus::Master);
        self.handshake_state = HandShakeState::PING;
        let mut arr = RespType::Array { length: 1, value: None};
        let ping = RespType::BulkStr { length: KW_PING.len(), value: Some(KW_PING.to_string()) };
        arr.add_item(ping);
        let _ = self.stream.write_all(&arr.to_bytes().unwrap());
    }

    fn get_replconf1(&self) -> Result<Option<Vec<u8>>, Vec<u8>> {
        // Return RESP bytes representing: REPLCONF listening-port <PORT>
        let mut arr = RespType::Array { length: 3, value: None }; 
        let resp_replconf = RespType::BulkStr { length: KW_REPLCONF.len(), value: Some(KW_REPLCONF.to_string()) };
        let resp_listening = RespType::BulkStr { length: KW_LISTENING_PORT.len(), value: Some(KW_LISTENING_PORT.to_string()) };
        // let port: String = Config::get().port.to_string();
        let stats = AppStates::get().get_host_stats()
            .ok_or_else(||
                CustomError::InternalError(ERR_HOST_STATS_NOT_INITIATED.to_string()))?;
        let port: String = stats.get_port()
            .ok_or_else(||
                CustomError::InternalError(ERR_MASTER_STATS_PORT_NOT_SET.to_string()))?
            .to_string();
        let resp_port = RespType::BulkStr { length: port.len(), value: Some(port) };
        arr.add_item(resp_replconf);
        arr.add_item(resp_listening);
        arr.add_item(resp_port);
        Ok(arr.to_bytes())
    }

    fn get_replconf2(&self) -> Result<Option<Vec<u8>>, Vec<u8>> {
        let mut arr = RespType::Array { length: 3, value: None }; 
        let resp_replconf = RespType::BulkStr { length: KW_REPLCONF.len(), value: Some(KW_REPLCONF.to_string()) };
        let resp_capa = RespType::BulkStr { length: KW_CAPA.len(), value: Some(KW_CAPA.to_string()) };
        let psync2 = "psync2";
        let resp_psync = RespType::BulkStr { length: psync2.len(), value: Some(psync2.to_string()) };
        arr.add_item(resp_replconf);
        arr.add_item(resp_capa);
        arr.add_item(resp_psync);
        Ok(arr.to_bytes())
    }

    fn get_psync(&self) -> Result<Option<Vec<u8>>, Vec<u8>> {
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
        Ok(arr.to_bytes())
    }
}
