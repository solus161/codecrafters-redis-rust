use std::io::{BufReader, BufRead, Error, Read};

pub fn consume_delimiter<R>(reader: &mut BufReader<R>) -> Result<bool, Box<dyn std::error::Error>>
where R: Read {
    // Consume the delimiter \r\n
    // TODO: what if \r\n not totaly match?
    let mut buf_delimiter = [0u8; 2];
    reader.read_exact(&mut buf_delimiter)?;
    let delimiter = String::from_utf8(buf_delimiter.to_vec())?;

    if &delimiter[..] == "\r\n" {
        return Ok(true)
    } else {
        return Ok(false)
    }
}

pub fn consume_start_length<R>(reader: &mut BufReader<R>) -> Result<bool, Error>
where R: Read {
    let next = reader.fill_buf()?[0];
    if next == b'$'{
        reader.consume(1);
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn read_digit<R>(reader: &mut BufReader<R>) -> Result<usize, Error> 
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
        Ok(buf_output.iter().fold(0, |acc, &b| {
            acc*10 + (b - b'0') as usize 
        }))
    }
}

pub fn read_cmd<R>(reader: &mut BufReader<R>, length: usize) -> Result<String, Box<dyn std::error::Error>>
where R: Read {
    let mut buf_cmd = vec![0u8; length];
    reader.read_exact(&mut buf_cmd)?;
    match String::from_utf8(buf_cmd) {
        Ok(s) => Ok(s),
        Err(e) => Err(Box::new(e))
    }
}
