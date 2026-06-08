use std::cell::RefCell;
use std::fmt::Display;
use std::fs;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{OnceLock};

use crate::exceptions::{ CustomError, ERR_RDB_CREATE };
use crate::Auth;

const RDB_FILE: &str = "redis-data";

pub struct AtomicOffset(AtomicI64);

impl AtomicOffset {
    pub fn new(v: i64) -> Self {
        Self(AtomicI64::new(v))
    }

    pub fn load(&self) -> i64 {
        self.0.load(Ordering::Relaxed)
    }

    pub fn add(&self, v: i64) -> i64{
        self.0.fetch_add(v, Ordering::Relaxed)
    }
}

impl Display for AtomicOffset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.load()) 
    }
}

// Config and config builder ____________________________________________________________
pub struct Configs {
    rdb_path: Option<String>,
    rdb_filename: Option<String>,
}

impl Configs {
    pub fn new() -> Self {
        Self {
            rdb_path: None,
            rdb_filename: None
        }
    }

    pub fn get_rdb_path(&self) -> Option<&String> {
        self.rdb_path.as_ref()
    }

    pub fn set_rdb_path(&mut self, rdb_path: &str) {
        self.rdb_path = Some(rdb_path.into())
    }

    pub fn get_rdb_filename(&self) -> Option<&String> {
        self.rdb_filename.as_ref()
    }

    pub fn set_rdb_filename(&mut self, rdb_filename: &str) {
        self.rdb_filename = Some(rdb_filename.into())
    }

    pub fn get_rdb_filepath(&self) -> (Option<&String>, Option<&String>) {
        (self.get_rdb_path(), self.get_rdb_filename())
    }

    pub fn build(self) {
        let _ = CONFIGS.set(self);
    }

    pub fn get() -> &'static Self {
        CONFIGS.get().expect("Config not initiated")
    }
}

pub struct ConfigsBuilder {
    configs: Configs
}

impl ConfigsBuilder {
    pub fn new() -> Self {
        Self {
            configs: Configs::new()
        }
    }

    pub fn with_rdb_path(mut self, path: &str) -> Self {
        // TODO: This need to be custom error, not Box dyn
        
        // Check path exists first
        match fs::exists(path) {
            Ok(true) => {
                self.configs.set_rdb_path(path);
            },
            Ok(false) => {
                panic!("Path not exists: {}", path)
            },
            Err(e) => {
                panic!("{}: {}", &ERR_RDB_CREATE, e.to_string())
            }
        };
        self
    }

    pub fn with_parse_config(mut self, args: &Vec<String>) -> Self {
        let mut i = 1;
        let mut dir: Option<String> = None;
        let mut filename: Option<String> = None;
        while i < args.len() {
            match &args[i] {
                s if s == "--dir" => {
                    dir = Some(args[i+1].clone());
                },
                s if s == "--dbfilename" => {
                    filename = Some(args[i+1].clone());
                },
                _ => {}
            };
            i += 2
        };

        if let Some(s) = dir {
            self.configs.set_rdb_path(&s);
        };

        if let Some(s) = filename {
            self.configs.set_rdb_filename(&s);
        };
        self
    }
        
    pub fn build(self) {
        self.configs.build();
    }
}

// Replication stats, for master and current server______________________________________
pub struct ReplStats {
    // This is used to store replication stats
    // of the host and its master if any
    host: Option<String>,
    port: Option<u16>,
    id: String,
    offset: AtomicOffset, // Offset off current server (slave)
}

impl ReplStats {
    pub fn new(host: Option<String>, port: Option<u16>) -> Self {
        Self {
            host,
            port,
            id: "?".to_string(),
            offset: AtomicOffset::new(-1)}
    }

    fn parse_replicaof(s: Option<String>) -> Option<Self> {
        if let Some(v) = s {
            let mut splitted = v.split(" ");
            let host = splitted.next().expect("Invalid replication host").to_string();
            let port: u16 = splitted.next().expect("Invalid replication port")
                .parse().expect("Invalid replication port");
            Some(Self::new (Some(host),Some(port)))
        } else {
            None
        }
    }

    pub fn start_bytes_count(&self) -> i64 {
        if self.offset.load() == -1 {
            self.offset.add(1)
        } else {
            -1
        }
    }
    
    pub fn add_bytes_count(&self, count: i64) -> i64 {
        if self.offset.load() > -1 {
            self.offset.add(count)
        } else {
            -1
        }
    } 

    pub fn get_host(&self) -> Option<&String> {
        self.host.as_ref()
    }

    pub fn get_port(&self) -> Option<&u16> {
        self.port.as_ref()
    }

    pub fn get_id(&self) -> &str {
         self.id.as_ref()
    }

    pub fn get_offset(&self) -> i64 {
        self.offset.load()
    }

}


// Other app status such as repl state___________________________________________________
pub struct AppStates {
    host_stats: Option<ReplStats>,
    master_stats: Option<ReplStats>,
}

impl AppStates {
    pub fn init(args: &Vec<String>) {
        let mut host: Option<String> = None;
        let mut port: Option<u16> = None;
        let mut replica_stats: Option<String> = None;

        let mut i = 1;
        while i < args.len() {
            let arg = args[i].to_lowercase();
            if arg == "--host" {
                host = Some(args[i+1].clone()) 
            } else if arg == "--port" || arg == "-p" {
                port = Some(args[i+1].parse().expect("Invalid port number"))
            } else if arg == "--replicaof" {
                replica_stats = Some(args[i+1].clone())
            };
            i += 2;
        };

        let host_resp = ReplStats::new(
            Some(host.unwrap_or("127.0.0.1".to_string())),
            Some(port.unwrap_or(6379)),
        );
        let master_repl = ReplStats::parse_replicaof(replica_stats);
        
        // After this, master start counting for bytes 
        host_resp.start_bytes_count();
        
        let _ = APP_STATES.set(Self{
            host_stats: Some(host_resp),
            master_stats: master_repl,
        });
    }

    pub fn get() -> &'static AppStates {
        APP_STATES.get().expect("App States not initialized")
    }

    pub fn is_slave(&self) -> bool {
        self.master_stats.is_some()
    }

    pub fn get_host_stats(&self) -> Option<&ReplStats> {
        self.host_stats.as_ref()
    }

    pub fn get_master_stats(&self) -> Option<&ReplStats> {
        self.master_stats.as_ref()
    }

    pub fn host_add_bytes_count(&self, count: i64) {
        match &self.host_stats {
            Some(stats) => {
                stats.add_bytes_count(count);
            },
            None => {}
        }
    }
}

static APP_STATES: OnceLock<AppStates> = OnceLock::new();
static CONFIGS: OnceLock<Configs> = OnceLock::new();
