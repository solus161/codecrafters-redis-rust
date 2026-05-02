use std::cell::RefCell;
use std::collections::VecDeque;
use std::error;
use std::io::{self,  Read, Write};
use std::net::TcpStream;
use std::rc::Rc;
use std::str::from_utf8;

use crate::cmd_handler::{Cmd, CmdHandler};
use crate::resp::{ RespType, RespParser };

#[derive(Debug)]
pub struct TcpClient {
    pub fd_key: u64,
    pub stream: TcpStream,
    pub resp_parser: RespParser,
    pub cmd_handler: Rc<RefCell<CmdHandler>>,
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
                    if let Some(r) = self.cmd_handler.borrow_mut().handle(cmd, self.fd_key) {
                        self.stream.write_all(r.as_bytes())?
                    }
                },
                None => break,
            }
        };
        Ok(())
    }
}
