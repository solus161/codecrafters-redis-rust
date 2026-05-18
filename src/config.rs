use std::fmt::Display;
use std::sync::OnceLock;
use std::rc::Rc;

pub enum Replication {
    Master{ id: String, offset: u64 },
    Slave{ host: String, port: u16 },
}

impl Replication {
    fn parse_role(s: Option<String>) -> Self {
        match s {
            None => Self::Master{
                id: "8371b4fb1155b71f4a04d3e1bc3e18c4a990aeeb".to_string(),
                offset: 0
            },
            Some(s) => {
                let mut splitted = s.split(" ");
                let host = splitted.next().expect("Invalid replication host").to_string();
                let port: u16 = splitted.next().expect("Invalid replication port")
                    .parse().expect("Invalid replication port");
                Self::Slave { host, port }
            }
        }
    }
}

impl Display for Replication {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Master{ id, offset } => write!(f, 
                "role:master\r\nmaster_replid:{}\r\nmaster_repl_offset:{}", id, offset),
            Self::Slave { .. } => write!(f, "role:slave"),
        }
    }
}

pub struct Config {
    pub host: String,
    pub port: u16,
    pub role: Replication,
}

impl Config {
    fn new(host: Option<String>, port: Option<u16>, role: Option<String>) -> Self {
        Self {
            host: host.unwrap_or("127.0.0.1".to_string()),
            port: port.unwrap_or(6379),
            role: Replication::parse_role(role) 
        }
    }

    pub fn init(args: Vec<String>) {
        let mut host: Option<String> = None;
        let mut port: Option<u16> = None;
        let mut role: Option<String> = None;

        let mut i = 1;
        while i < args.len() {
            let arg = args[i].to_lowercase();
            if arg == "--host" {
                host = Some(args[i+1].clone()) 
            } else if arg == "--port" || arg == "-p" {
                port = Some(args[i+1].parse().expect("Invalid port number"))
            } else if arg == "--replicaof" {
                role = Some(args[i+1].clone())
            };
            i += 2;
        };
        let _ = CONFIG.set(Self::new(host, port, role));
    }

    pub fn get() -> &'static Config {
        CONFIG.get().expect("Config not initialized")
    }

    pub fn get_info(&self) -> String {
        format!("{}", self.role.to_string())
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();
