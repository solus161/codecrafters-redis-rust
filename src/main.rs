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
mod cmd_handler;
mod client;

use crate::client::{TcpClient, BUFFER_SIZE};
use crate::cmd_handler::CmdHandler;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Uncomment the code below to pass the first stage
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();
    listener.set_nonblocking(true).unwrap();
    let listener_fd = listener.as_raw_fd();
    let listener_fd_u64 = listener_fd as u64;
    let cmd_handler = Rc::new(RefCell::new(CmdHandler::new()));

    // Get fd on epoll event
    let epoll_fd = epoll::epoll_create().expect("Error creating epoll queue");
    
    epoll::add_interest(epoll_fd, listener_fd, epoll::get_epoll_event_read(listener_fd_u64))?;
    
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
                1000 as libc::c_int,
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
                            // println!("New client: {}", addr);

                            // Add the stream fd to epoll watch queue
                            let stream_key = stream.as_raw_fd();
                            epoll::add_interest(
                                epoll_fd,
                                stream_key,
                                epoll::get_epoll_event_read(stream_key as u64))?;
                            clients.insert(
                                stream_key.try_into().unwrap(),
                                TcpClient::new(stream, Rc::clone(&cmd_handler)));
                            // println!("Registered epoll with key {}", stream_key);
                        },
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {eprintln!("{}", e)},
                        Err(e) => eprintln!("Couldn't accept: {}", e),
                    };
                    
                    // println!("Try to modify interest");
                    // Register epoll queue with its own key again
                    epoll::modify_interest(
                        epoll_fd, listener_fd.try_into().unwrap(), 
                        epoll::get_epoll_event_read(listener_fd as u64))?;
                },
                
                // St else, may be current client
                key => {
                    //println!("key in events {}", key);
                    // println!{"Package with key {}", key};
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
                                        // println!("Registered epoll with key {} again", key);
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
                }
            }
        }
    }
}

