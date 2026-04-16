#![allow(unused_imports)]
use std::net::TcpListener;
use std::io::{Read, Write, BufReader, BufRead};

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    // Uncomment the code below to pass the first stage
    let listener = TcpListener::bind("127.0.0.1:6379").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(mut _stream) => {
                println!("accepted new connection");
                let mut reader = BufReader::new(_stream.try_clone().unwrap());
                let mut line = String::new();
                //let mut not_end_signal = true;

                loop {
                    match reader.read_line(&mut line) {
                        Ok(_) => {
                            match &line[..] {
                                "PING" => {
                                    _stream.write_all("+PONG\r\n".as_bytes()).unwrap();                        
                                },
                                _ => {println!("Something else")}
                            };
                        },
                        Err(e) => panic!("{}", e),
                    };
                    let _ = _stream.flush();
                }
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
