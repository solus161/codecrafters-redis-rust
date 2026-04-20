#![allow(unused_imports)]
use std::collections::HashMap;
use std::net::{ TcpListener, TcpStream };
use std::io::{self, BufRead, BufReader, Error, ErrorKind, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::collections::VecDeque;
use std::str::from_utf8;
use libc;

#[macro_use]
mod utils;
mod epoll;
mod client;

use crate::client::{TcpClient, BUFFER_SIZE_USIZE, BUFFER_SIZE_I32};

const LISTENER_KEY: u64 = 100;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Uncomment the code below to pass the first stage
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();
    listener.set_nonblocking(true).unwrap();
    let listener_fd = listener.as_raw_fd();

    // Get fd on epoll event
    let epoll_fd = epoll::epoll_create().expect("Error creating epoll queue");
    
    epoll::add_interest(epoll_fd, listener_fd, epoll::get_epoll_event_read(LISTENER_KEY))?;
    
    let mut events: Vec<libc::epoll_event> = Vec::with_capacity(BUFFER_SIZE_USIZE);
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
                BUFFER_SIZE_I32,
                1000 as libc::c_int,
            )
        ) {
            Ok(v) => v,
            Err(e) => panic!("Error during epoll wait: {}", e),
        };
 
        unsafe { events.set_len(res as usize)};

        for ev in &events {
            match ev.u64 {
                // New client comming in
                100 => {
                    match listener.accept() {
                        Ok((stream, addr)) => {
                            stream.set_nonblocking(true)?;
                            println!("New client: {}", addr);

                            // Add the stream fd to epoll watch queue
                            epoll::add_interest(
                                epoll_fd,
                                stream.as_raw_fd(), epoll::get_epoll_event_read(LISTENER_KEY))?;
                            clients.insert(stream.as_raw_fd() as u64, TcpClient::new(stream));
                            println!("Registered epoll with key {}", LISTENER_KEY);
                        },
                        Err(e) => eprintln!("Coundn't accept: {}", e),
                    };

                    // Register epoll with key 100 again
                    epoll::modify_interest(
                        epoll_fd, listener_fd, 
                        epoll::get_epoll_event_read(100))?;
                },
                
                // St else, may be current client
                key => {
                    //println!("key in events {}", key);
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
                                            epoll_fd, client.stream.as_raw_fd(), 
                                            epoll::get_epoll_event_read(key))?;
                                        println!("Registered epoll with key {} again", key);
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

