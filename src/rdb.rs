use std::collections::HashMap;
use std::io::{BufReader, Read};
use std::fs::{File};
use std::usize;

use crate::cmd_handler::{StoreItem, StoreValue};
use crate::exceptions::CustomError;

pub struct Rdb {
    _path: String,
    buf: BufReader<File>
}

impl Rdb {
    pub fn open(path: String) -> Option<Self> {
        // Checking for existing path must be done by Configs
        if let Ok(file) = File::open(&path) {
            let buf = BufReader::new(file);
            Some(Self { _path: path, buf })
        } else {
            None
        }
    }

    pub fn read(&mut self) -> Result<HashMap<String, StoreItem>, CustomError> {
        let msg = "Error reading file";
        // Read magic string REDIS
        let mut magic = [0u8; 5];
        self.buf.read_exact(&mut magic).expect(msg);

        // Read version nbr
        let mut version = [0u8; 4];
        self.buf.read_exact(&mut version).expect(msg);

        let mut data = HashMap::new();
        let mut opcode = [0u8; 1];
        loop {
            self.buf.read_exact(&mut opcode).expect(msg);
            match opcode[0] {
                0xFA => {
                    let _key = self.parse_string()?;
                    let _value = self.parse_string()?;
                },
                0xFE => {
                    let _size = self.parse_length()?;
                },
                0xFB => {
                    let _hash_size = self.parse_length()?;
                    let _hash_size_exp = self.parse_length()?;
                },
                0xFC => {
                    let exp_ms = self.parse_exp_ms()?;
                    let (key, value) = self.parse_type_string()?;
                    let store_value = StoreValue::Str(value);
                    let store_item = StoreItem::new(store_value, Some(exp_ms));
                    data.insert(key, store_item);
                },
                0xFD => {
                    let exp_s = self.parse_exp_s()?;
                    let (key, value) = self.parse_type_string()?;
                    let store_value = StoreValue::Str(value);
                    let store_item = StoreItem::new(store_value, Some(exp_s));
                    data.insert(key, store_item);
                },
                0xFF => break,
                _ => {
                    // TODO: type may not be 0x00
                    let key = self.parse_string()?;
                    let value = self.parse_string()?;
                    let store_value = StoreValue::Str(value);
                    let store_item = StoreItem::new(store_value, None);
                    data.insert(key, store_item);
                }
            }
        };
        Ok(data)
    }

    fn parse_length(&mut self) -> Result<u32, CustomError> {
        let mut byte_0 = [0u8; 1];
        
        // Check first 2 bits
        self.buf.read_exact(&mut byte_0)?;
        match byte_0[0] & 0b11000000 {
            0b00000000 => {
                // No need to mask first 2 bits
                Ok(byte_0[0].into())
            },
            0b01000000 => {
                // Mask first 2 bits
                let len_0 = (byte_0[0] & 0b00111111) as u16;

                // Read next byte
                let mut len_1 = [0u8; 1];
                self.buf.read_exact(&mut len_1)?;
                Ok(((len_0 << 8) | (len_1[0] as u16)).into())
            },
            0b10000000 => {
                let mut byte_1 = [0u8; 4];
                self.buf.read_exact(&mut byte_1)?;
                Ok(u32::from_be_bytes(byte_1))
            },
            _ => {
                // May be other type 
                Err(CustomError::RDBParsingError) 
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, CustomError> {
        let mut byte_0 = [0u8; 1];
        self.buf.read_exact(&mut byte_0)?;

        // First case: string with no format type
        match byte_0[0] & 0b11000000 {
            0b00000000 => {
                // Just string
                // Check string length
                let length = (byte_0[0] & 0b00111111) as usize;
                let mut output = vec![0u8; length];
                self.buf.read_exact(&mut output)?; 
                return Ok(String::from_utf8(output)?)
            },
            _ => { }
        };

        // Second case: string with format type
        match byte_0[0] & 0xFF {
            0xC0 => {
                //next byte is integer
                let mut byte_1 = [0u8; 1];
                self.buf.read_exact(&mut byte_1)?;
                Ok(i16::from(byte_1[0]).to_string())               
            },
            0xC1 => {
                // Next 2 bytes, little endian
                let mut byte_1 = [0u8; 2];
                self.buf.read_exact(&mut byte_1)?;
                Ok(i16::from_le_bytes(byte_1).to_string())
            },
            0xC2 => {
                // Next 2 bytes, little endian
                let mut byte_1 = [0u8; 4];
                self.buf.read_exact(&mut byte_1)?;
                Ok(i32::from_le_bytes(byte_1).to_string())
            },
            _ => Err(CustomError::RDBParsingError)
        }
    }

    fn parse_exp_ms(&mut self) -> Result<u64, CustomError> {
        let mut byte_0 = [0u8; 8];
        self.buf.read_exact(&mut byte_0)?;
        Ok(u64::from_le_bytes(byte_0))
    }

    fn parse_exp_s(&mut self) -> Result<u64, CustomError> {
        let mut byte_0 = [0u8; 4];
        self.buf.read_exact(&mut byte_0)?;
        Ok((u32::from_le_bytes(byte_0)) as u64)
    }

    fn parse_type_string(&mut self) -> Result<(String, String), CustomError> {
        let mut data_type = [0u8; 1];
        self.buf.read_exact(&mut data_type)?; 

        match data_type[0] {
            0x00 => {
                let key = self.parse_string()?;
                let value = self.parse_string()?;
                Ok((key, value))
            },
            _ => Err(CustomError::RDBParsingError)
        }
    }
}
