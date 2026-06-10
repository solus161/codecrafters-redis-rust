use std::cell::RefCell;
use std::env;
use std::collections::HashMap;
use std::net::{ TcpListener, TcpStream };
use std::io::{self, Write}; 
use std::os::fd::{AsRawFd};
use std::rc::Rc;
use libc;

mod app_state;
#[macro_use]
mod utils;
mod epoll;
mod cmd_builder;
mod cmd_handler;
mod client;
mod resp;
mod replication;
mod tests;
mod exceptions;
mod rdb;
mod custom_data;
mod geohash;
mod auth;

use crate::app_state::{AppStates, Configs, ConfigsBuilder};
use crate::client::{TcpClient, BUFFER_SIZE};
use crate::epoll::{timer_create_fd};
use crate::exceptions::{
    ERR_CREATING_EPOLL, ERR_HOST_STATS_NOT_INITIATED,
    ERR_MASTER_STATS_HOST_NOT_SET, ERR_MASTER_STATS_PORT_NOT_SET};
use crate::cmd_handler::CmdHandler;
use crate::rdb::Rdb;
use crate::replication::ClientTable;
use crate::auth::{Auth};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parsing args for port
    let args: Vec<String> = env::args().collect();
    AppStates::init(&args);
    let app_state = AppStates::get();

    // Parsing args for configs
    ConfigsBuilder::new().with_parse_config(&args).build();

    // Fd for listener 
    let host_stats = app_state.get_host_stats().expect(ERR_HOST_STATS_NOT_INITIATED);
    let host = host_stats.get_host().unwrap();
    let port = host_stats.get_port().unwrap();
    let listener = TcpListener::bind(format!("{}:{}", host, port)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let listener_fd = listener.as_raw_fd();
    let listener_fd_u64 = listener_fd as u64;
    
    // To store all clients or a master
    let mut clients: HashMap<u64, TcpClient> = HashMap::new();

    // Fd for timer
    let timer_fd = timer_create_fd();

    let cmd_handler = Rc::new(
        RefCell::new(CmdHandler::new(timer_fd))
        );
    
    // Load RDB if any
    if let (Some(path), Some(filename)) = Configs::get().dbfilepath() {
        let filepath = format!("{}/{}", path, filename);
        if let Some(mut rdb) = Rdb::open(filepath) {
            if let Ok(data) = rdb.read() {
                cmd_handler.borrow_mut().load_data(data);
            };
        };
        
    }

    // Get fd on epoll event
    let epoll_fd = epoll::epoll_create().expect(ERR_CREATING_EPOLL);

    // Add a master if in slave
    if let Some(repl_stats) =  app_state.get_master_stats() {
        println!("Run as replica");
        let host = repl_stats.get_host().expect(ERR_MASTER_STATS_HOST_NOT_SET);
        let port = repl_stats.get_port().expect(ERR_MASTER_STATS_PORT_NOT_SET);
        // Connect to master
        let master = TcpStream::connect(format!("{}:{}", host, port))
            .expect("Cannot connect to master");
        let master_fd = master.as_raw_fd() as u64;

        epoll::add_interest(
            epoll_fd, master.as_raw_fd(), 
            epoll::get_epoll_event_read(master_fd))?;

        // Assign a default credential
        Auth::get().borrow_mut().authenticate(&master_fd, None, None)
            .expect("Error assigning default credential");

        // Put master to the client table
        let mut client_master = TcpClient::new(
            master_fd,
            epoll_fd,
            master,
            Rc::clone(&cmd_handler));

        // Init handshake with master
        client_master.init_handshake();
        
        // Store master
        clients.insert(master_fd, client_master);

        // Initiate handshake
        // client_master.init_handshake();
        // clients.insert(
        //     master_fd.try_into().unwrap(),
        //     client_master
        //     );
    };

    // Add listener to epoll for changes
    epoll::add_interest(epoll_fd, listener_fd, epoll::get_epoll_event_read(listener_fd_u64))?;
    
    // Add timer to epoll for changes
    epoll::add_interest(epoll_fd, timer_fd, epoll::get_epoll_event_read(timer_fd as u64))?;

    let mut events: Vec<libc::epoll_event> = Vec::with_capacity(BUFFER_SIZE as usize);

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

                            // Assign default credential
                            // this will not response anything
                            // if the default credential requires pass
                            // TODO
                            let client_id = stream_key as u64;
                            Auth::get().borrow_mut().authenticate(&client_id, None, None)
                                .expect("Error assigning default credential");

                            // Add to table holding tcp client
                            clients.insert(
                                client_id, 
                                TcpClient::new(
                                    stream_key as u64,
                                    epoll_fd,
                                    stream, 
                                    Rc::clone(&cmd_handler)));
                        },
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {eprintln!("{}", e)},
                        Err(e) => eprintln!("Couldn't accept: {}", e),
                    };
                    // Register epoll queue with its own key again
                    epoll::modify_interest(
                        epoll_fd, listener_fd, 
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
                
                // St else, may be current client sending st
                key => {
                    let client_table_rc = ClientTable::get();
                    if let Some(client) = clients.get_mut(&key) {
                        let mut disconnected = false;
                        // Bit mask of event type of an epoll event
                        let events: u32 = ev.events;
                        match events {
                            v if v as i32 & libc::EPOLLIN == libc::EPOLLIN => {
                                match client.read_socket() {
                                    Ok(()) => {
                                        // Re-register epoll. ENOENT means the command parked
                                        // the client (WAIT/BLPOP/XREAD BLOCK called remove_interest),
                                        // so skip re-registering in that case.
                                        if let Err(e) = epoll::modify_interest(
                                            epoll_fd, key as i32,
                                            epoll::get_epoll_event_read(key))
                                        {
                                            if e.kind() != io::ErrorKind::NotFound {
                                                return Err(e.into());
                                            }
                                        }
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
                            client_table_rc.borrow_mut().remove_client(&key);
                            clients.remove(&key);
                            Auth::get().borrow_mut().remove_auth(&key);
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
        cmd_handler.borrow_mut().serve_backlog_list(); 
        cmd_handler.borrow_mut().serve_backlog_stream();

        // BLPOP responses gathered, now flush
        for res in cmd_handler.borrow_mut().response_queue.drain(..) { 
            let (client_id, message) = res;
            let _ = clients.get_mut(&client_id).unwrap().stream.write_all(&message);
        };
    }
}
