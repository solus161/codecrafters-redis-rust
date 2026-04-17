#![allow(unused_imports)]
use std::net::TcpListener;
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Write};

mod utils;

use crate::utils::resp_parser;

fn main() {
    // Uncomment the code below to pass the first stage
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(mut _stream) => {
                let mut reader = BufReader::new(_stream.try_clone().unwrap());
                let mut buf_start = [0u8; 1];
                
                // Parse into cmd first, handle later
                let mut args: Vec<String> = Vec::new();

                loop {
                    // Read till got the start of command 
                    match reader.read_exact(&mut buf_start) {
                        Ok(_) => {
                            match buf_start[0] {
                                // Detect a start of command
                                b'*' => {
                                    buf_start[0] = 0;
                            
                                    // Read following args length
                                    let cmd_size = match resp_parser::read_digit(&mut reader){
                                        Ok(size) => if size > 0 { size } else { break }, 
                                        
                                        // Malformed pattern, search for next *
                                        Err(_) => continue,
                                    };

                                    // TODO: properly handle error or malformed sequence
                                    let _ = resp_parser::consume_delimiter(&mut reader);
                                    
                                    args.clear();
                                    for _ in 0..cmd_size {
                                        // Read the $ start of arg length
                                        let _ = resp_parser::consume_start_length(&mut reader);

                                        // Read the arg length
                                        let length = resp_parser::read_digit(&mut reader).unwrap();

                                        let _ = resp_parser::consume_delimiter(&mut reader);

                                        // Read cmd
                                        let cmd = resp_parser::read_cmd(&mut reader, length).unwrap();
                                        args.push(cmd);
                                        
                                        let _ = resp_parser::consume_delimiter(&mut reader);
                                    };

                                    // Handling args
                                    for arg in args.iter() {
                                        match &arg[..] {
                                            "PING" => {
                                                let _ = _stream.write_all("+PONG\r\n".as_bytes());
                                            },
                                            _ => continue
                                        }
                                    }

                                    let _ = _stream.flush();

                                },
                                _ => continue
                            };
                        },
                        Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
                        Err(_) => break,
                    };
                }
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}


