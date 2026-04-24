use std::collections::HashMap;

#[derive(Debug, Clone)]
enum StoredValue {
Str(String),
    Int(i64),
}

#[derive(Debug)]
pub struct CmdHandler {
    data: HashMap<String, StoredValue>
}

impl CmdHandler {
    pub fn new() -> Self{
        Self { data: HashMap::new() }
    }

    pub fn handle(&mut self, mut args: Vec<String>) -> Option<String> {
        if args.is_empty() {
            return None
        };

        let mut args_iter = args.drain(..);
        let cmd = args_iter.next().unwrap().to_lowercase();
        
        println!("Cmd Handler trying to match {}", &cmd);

        match cmd {
            s if s == "ping" => {
                Self::cmd_ping()
            },
            s if s == "echo" => {
                Self::cmd_echo(args_iter.next().unwrap())    
            },
            s if s == "set" => {
                // TODO: handling case where there is now arg
                let key = args_iter.next().unwrap();
                let value = args_iter.next().unwrap();
                Some(self.cmd_set(key, value))
            },
            s if s == "get" => {
                let key = args_iter.next().unwrap();
                Some(self.cmd_get(key))
            }
            _ => None
        }
    }
    
    // TODO: all these returns should be of Result<>

    fn serialize(string: String) -> String {
        format!("${}\r\n{}\r\n", string.len(), string)
    }

    fn response_ok() -> String {
        "+OK\r\n".to_string()
    }

    fn cmd_ping() -> Option<String> {
        Some("+PONG\r\n".to_string())
    }

    fn cmd_echo(arg: String) -> Option<String> {
        Some(
            Self::serialize(arg)
        )
    }

    fn cmd_set(&mut self, key: String, value: String) -> String {
        self.data.insert(key, StoredValue::Str(value));
        Self::response_ok()
    }

    fn cmd_get(&self, key: String) -> String {
        match self.data.get(&key).cloned() {
            Some(stored_value) => {
                match stored_value {
                    StoredValue::Str(value) => {
                        Self::serialize(value)
                    },
                    
                    // TODO: implement for Int
                    StoredValue::Int(_) => {
                        "Int value not implemented".to_string() 
                    },
                }
            },
            None => "$-1\r\n".to_string()
        } 
    }
}
