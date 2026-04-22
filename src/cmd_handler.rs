pub struct Cmd {
    args: Vec<String>,
}

impl Cmd {
    pub fn new(args: Vec<String>) -> Self {
        Self { args }
    }

    pub fn handle(&mut self) -> Option<String> {
        if self.args.len() == 0 {
            return None
        };

        let mut args_iter = self.args.drain(..);
        let cmd = args_iter.next().unwrap().to_lowercase();
        
        println!("Cmd Handler trying to match {}", &cmd);

        match cmd {
            s if s == "ping" => {
                Some(Self::serialize("+PONG".to_string()))
            },
            s if s == "echo" => {
                Some(
                    Self::serialize(
                        args_iter.fold(String::new(), |acc, s| acc + &s))
            )},
            _ => None
        }
    }

    fn serialize(string: String) -> String {
        format!("${}\r\n{}\r\n", string.len(), string)
    }
}

