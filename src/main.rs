#![allow(unused_imports)]
use std::cell::RefCell;
use std::collections::HashMap;
use std::net::{ TcpListener, TcpStream };
use std::io::{self, BufRead, BufReader, Error, ErrorKind, Read, Write}; 
use std::os::fd::{AsRawFd, RawFd};
use std::collections::VecDeque;
use std::rc::Rc;
use std::str::from_utf8;
use libc;

#[macro_use]
mod utils;
mod epoll;
mod cmd_builder;
mod cmd_handler;
mod client;
mod resp;
mod tests;

use crate::client::{TcpClient, BUFFER_SIZE};
use crate::epoll::{get_epoll_event_read, timer_create_event, timer_create_fd};
use crate::resp::RespParser;
use crate::cmd_handler::CmdHandler;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Fd for listener 
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();
    listener.set_nonblocking(true).unwrap();
    let listener_fd = listener.as_raw_fd();
    let listener_fd_u64 = listener_fd as u64;
    
    // Fd for timer
    let timer_fd = timer_create_fd();

    let cmd_handler = Rc::new(RefCell::new(CmdHandler::new(timer_fd)));
    
    // Get fd on epoll event
    let epoll_fd = epoll::epoll_create().expect("Error creating epoll queue");
    
    // Add listener to epoll for changes
    epoll::add_interest(epoll_fd, listener_fd, epoll::get_epoll_event_read(listener_fd_u64))?;
    
    // Add timer to epoll for changes
    epoll::add_interest(epoll_fd, timer_fd, epoll::get_epoll_event_read(timer_fd as u64))?;

    let mut events: Vec<libc::epoll_event> = Vec::with_capacity(BUFFER_SIZE as usize);
    let mut clients: HashMap<u64, TcpClient> = HashMap::new();

    loop {
        events.clear();
        let res = match syscall!(
            // epoll_wait need an epoll_fd and a raw pointer for events buffer
            // read up to BUFFER_SIZE events
            // timeout after 1000ms if no event fired
            epoll_wait(
                epoll_fd,
                events.as_mut_ptr() as *mut libc::epoll_event,
                BUFFER_SIZE,
                -1 as libc::c_int,
            )
        ) {
            Ok(v) => v,
            Err(e) => panic!("Error during epoll wait: {}", e),
        };
 
        unsafe { events.set_len(res as usize)};

        for ev in &events {
            let ev_key = ev.u64;
            match ev_key {
                // New client comming in
                _key if _key == listener_fd_u64 => {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(true)?;

                            // Add the stream fd to epoll watch queue
                            let stream_key = stream.as_raw_fd();
                            epoll::add_interest(
                                epoll_fd,
                                stream_key,
                                epoll::get_epoll_event_read(stream_key as u64))?;
                            clients.insert(
                                stream_key.try_into().unwrap(),
                                TcpClient::new(
                                    stream_key.try_into().unwrap(),
                                    stream, 
                                    Rc::clone(&cmd_handler)));
                        },
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {eprintln!("{}", e)},
                        Err(e) => eprintln!("Couldn't accept: {}", e),
                    };
                    // Register epoll queue with its own key again
                    epoll::modify_interest(
                        epoll_fd, listener_fd.try_into().unwrap(), 
                        epoll::get_epoll_event_read(listener_fd as u64))?;
                },
                
                // Timer triggered
                _key if _key == timer_fd as u64 => {
                    // Clear timer, reading the fd clears the readable state
                    let mut buf = [0u8; 8];
                    unsafe { libc::read(timer_fd, buf.as_mut_ptr() as *mut _, 8) };

                    // If deadline is not served, client_id should receive a NullBulkStr
                    cmd_handler.borrow_mut().callback_deadline_expire();
                },
                
                // St else, may be current client
                key => {
                    if let Some(client) = clients.get_mut(&key) {
                        let mut disconnected = false;
                        // Bit mask of event type of an epoll event
                        let events: u32 = ev.events;
                        println!("epoll event key {} triggered", key);
                        match events {
                            v if v as i32 & libc::EPOLLIN == libc::EPOLLIN => {
                                match client.read_socket() {
                                    Ok(()) => {
                                        // Register the epoll again
                                        epoll::modify_interest(
                                            epoll_fd, key as i32, 
                                            epoll::get_epoll_event_read(key))?;
                                    },
                                    Err(boxed_e) => {
                                        println!("Error with client fd {}: {:?}", key, boxed_e);
                                        disconnected = true;
                                    }
                                };

                                                            },
                            //v if v as i32 & libc::EPOLLOUT== libc::EPOLLOUT => {
                            //    stream.write_cb(key, epoll_fd)?;
                            //    to_delete = Some(key);
                            //},
                            v => println!("Unexpected events: {}", v),
                        };
                        if disconnected {
                            let _ = epoll::remove_interest(epoll_fd, client.stream.as_raw_fd());
                            clients.remove(&key);
                        };
                    };
                },

                
            }
        };
        
        // All events processed, now processing waiting queue for BLPOP
                
        // After a batch/cycle, match available blpop and item
        // For example: B -> BLPOP 0 at t0
        // A -> RPUSH key "a" at t1
        // end cycle, A and B must be matched, 
        // not waiting till next cycle
        cmd_handler.borrow_mut().serve_queue(); 

        // BLPOP responses gathered, now flush
        for res in cmd_handler.borrow_mut().response_queue.drain(..) { 
            let (client_id, message) = res;
            let _ = clients.get_mut(&client_id).unwrap()
                .stream.write_all(message.as_bytes());
        };
    }
}

