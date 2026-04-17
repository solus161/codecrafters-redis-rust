#![allow(unused_imports)]
use std::net::TcpListener;
use std::io::{BufRead, BufReader, Error, ErrorKind, Read, Write};


fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

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
                                    let cmd_size = match read_digit(&mut reader){
                                        Ok(size) => if size > 0 { size } else { break }, 
                                        
                                        // Malformed pattern, search for next *
                                        Err(_) => continue,
                                    };

                                    // TODO: properly handle error or malformed sequence
                                    let _ = consume_delimiter(&mut reader);
                                    
                                    args.clear();
                                    for _ in 0..cmd_size {
                                        // Read the $ start of arg length
                                        let _ = consume_start_length(&mut reader);

                                        // Read the arg length
                                        let length = read_digit(&mut reader).unwrap();

                                        let _ = consume_delimiter(&mut reader);

                                        // Read cmd
                                        let cmd = read_cmd(&mut reader, length).unwrap();
                                        args.push(cmd);
                                        
                                        let _ = consume_delimiter(&mut reader);
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

fn consume_delimiter<R>(reader: &mut BufReader<R>) -> Result<bool, Box<dyn std::error::Error>>
where R: Read {
    // Consume the delimiter \r\n
    // TODO: what if \r\n not totaly match?
    let mut buf_delimiter = [0u8; 4];
    reader.read_exact(&mut buf_delimiter)?;
    let delimiter = String::from_utf8(buf_delimiter.to_vec())?;

    if &delimiter[..] == "\r\n" {
        return Ok(true)
    } else {
        return Ok(false)
    }
}

fn consume_start_length<R>(reader: &mut BufReader<R>) -> Result<bool, Box<dyn std::error::Error>>
where R: Read {
    let next = reader.fill_buf()?[0];
    if next == b'$'{
        reader.consume(1);
        Ok(true)
    } else {
        Ok(false)
    }
}

fn read_digit<R>(reader: &mut BufReader<R>) -> Result<usize, Error> 
where R: Read {
    // Read till non digit char
    let mut buf_output: Vec<u8> = Vec::new();
    let mut buf_digit = [0u8; 1];

    loop {
        // Take a look at next byte
        let next = reader.fill_buf()?[0];
        
        // Only consume if it is digit
        if next.is_ascii_digit() {
            let _ = reader.read_exact(&mut buf_digit);
            buf_output.push(buf_digit[0]);
        } else {
            break;
        }
    };
    
    if buf_output.len() == 0 {
        return Ok(0)
    } else {
        Ok(buf_output.iter().fold(0u32, |acc, &b| {
            acc*10 + (b - b'0') as u32
        }) as usize)
    }
}

fn read_cmd<R>(reader: &mut BufReader<R>, length: usize) -> Result<String, Box<dyn std::error::Error>>
where R: Read {
    let mut buf_cmd = vec![0u8; length];
    reader.read_exact(&mut buf_cmd)?;
    match String::from_utf8(buf_cmd) {
        Ok(s) => Ok(s),
        Err(e) => Err(Box::new(e))
    }
}
