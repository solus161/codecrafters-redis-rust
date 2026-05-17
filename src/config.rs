use std::sync::OnceLock;
use std::rc::Rc;

pub struct Config {
    pub host: String,
    pub port: u16,
}

impl Config {
    fn new(host: Option<String>, port: Option<u16>) -> Self {
        Self {
            host: host.unwrap_or("localhost".to_string()),
            port: port.unwrap_or(6379)
        }
    }

    pub fn init(args: Vec<String>) {
        let mut host: Option<String> = None;
        let mut port: Option<u16> = None;

        let mut i = 1;
        while i < args.len() {
            if args[i].to_lowercase() == "--host" {
                host = Some(args[i+1].clone()) 
            } else if args[i].to_lowercase() == "--port" {
                port = Some(args[i+1].parse().expect("Invalid port number"))
            };
            i += 2;
        };
        let _ = CONFIG.set(Self::new(host, port));
    }

    pub fn get() -> &'static Config {
        CONFIG.get().expect("Config not initialized")
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();
