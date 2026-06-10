use std::fmt::{Display};
use std::fs::{self};
use std::io::Write;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{OnceLock};

use crate::exceptions::{ CustomError };

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
// Flags for config parsing
const KW_DIR: &str = "--dir";
const KW_DBFILENAME: &str = "--dbfilename";
const KW_APPENDONLY: &str = "--appendonly";
const KW_APPENDDIRNAME: &str = "--appenddirname";
const KW_APPENDFILENAME: &str = "--appendfilename";
const KW_APPENDFSYNC: &str = "--appendfsync";

// Enum for AppendFsync
#[derive(Debug)]
pub enum AppendFsync {
    Always,
    EverySec
}

impl AppendFsync {
    pub const fn get_str(&self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::EverySec => "everysec"
        }
    }
}

impl Display for AppendFsync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Always => write!(f, "always"),
            Self::EverySec => write!(f, "everysec")
        }
    }
}

impl TryFrom<&str> for AppendFsync {
    type Error = CustomError;

    fn try_from(value: &str) -> Result<Self, CustomError> {
        match value {
            "always" => Ok(AppendFsync::Always),
            "everysec" => Ok(AppendFsync::EverySec),
            _ => Err(CustomError::InvalidArgument("Invalid arg".to_string()))
        }
    }
}


// Configs
#[derive(Debug)]
pub struct Configs {
    path: Option<String>,
    dbfilename: Option<String>, // rdb file name
    appendonly: bool,           // there are for aof persistence
    appenddirname: String,
    appendfilename: String,
    appendfsync: AppendFsync,
}

impl Configs {
    pub fn new() -> Self {
        Self {
            path: Some("/app".to_string()),
            dbfilename: None,
            appendonly: false,
            appenddirname: "appendonlydir".to_string(),     // relative path
            appendfilename: "appendonly.aof".to_string(),
            appendfsync: AppendFsync::EverySec
        }
    }

    pub fn get_attr(&self, key: &str) -> Result<&str, CustomError> {
        match key {
            "dir" => Ok(self.path.as_ref().map_or("none", |s| s.as_ref())),
            "dbfilename" => Ok(self.dbfilename.as_ref().map_or("", |f| f.as_ref())),
            "appendonly" => Ok(self.appendonly.then(|| {"yes"}).unwrap_or("no")),
            "appenddirname" => Ok(self.appenddirname.as_ref()),
            "appendfilename" => Ok(self.appendfilename.as_ref()),
            "appendfsync" => Ok(self.appendfsync.get_str().as_ref()),
            _ => Err(CustomError::UnsupportedCmd("Unsupported".to_string()))
        }
    }

    pub fn path(&self) -> Option<&String> {
        self.path.as_ref()
    }

    pub fn set_path(&mut self, path: &str) {
        self.path = Some(path.into())
    }

    pub fn dbfilename(&self) -> Option<&String> {
        self.dbfilename.as_ref()
    }

    pub fn set_dbfilename(&mut self, dbfilename: &str) {
        self.dbfilename = Some(dbfilename.into())
    }

    pub fn dbfilepath(&self) -> (Option<&String>, Option<&String>) {
        (self.path(), self.dbfilename())
    }

    pub fn appendonly(&self) -> bool {
        self.appendonly
    }

    pub fn set_appendonly(&mut self, value: bool) {
        self.appendonly = value
    }

    pub fn appenddirname(&self) -> &str {
        self.appenddirname.as_ref()
    }

    pub fn set_appenddirname(&mut self, value: &str) {
        self.appenddirname = value.to_string()
    }

    pub fn appendfilename(&self) -> &str {
        self.appendfilename.as_ref()
    }

    pub fn set_appendfilename(&mut self, value: &str) {
        self.appendfilename = value.to_string()
    }

    pub fn set_appendfsync(&mut self, value: AppendFsync) {
        self.appendfsync = value
    }

    pub fn build(self) {
        let _ = CONFIGS.set(self);
    }

    pub fn get() -> &'static Self {
        CONFIGS.get().expect("Config not initiated")
    }
}

// ConfigBuilder
pub struct ConfigsBuilder {
    configs: Configs
}

impl ConfigsBuilder {
    pub fn new() -> Self {
        Self {
            configs: Configs::new()
        }
    }

    pub fn with_parse_config(mut self, args: &Vec<String>) -> Self {
        let mut i = 1;
        let yes_no_to_bool = |s: &str| {
            match s {
                "yes" => true,
                "no" => false,
                _ => panic!("Unsupported value")
            } 
        };
        
        while i < args.len() {
            match args[i].as_str() {
                KW_DIR => {
                    self.configs.set_path(&args[i+1]);
                },
                KW_DBFILENAME => {
                    self.configs.set_dbfilename(&args[i+1]);
                },
                KW_APPENDONLY => {
                    self.configs.set_appendonly(yes_no_to_bool(&args[i+1]))
                },
                KW_APPENDDIRNAME => {
                    self.configs.set_appenddirname(&args[i+1]);
                },
                KW_APPENDFILENAME => {
                    self.configs.set_appendfilename(&args[i+1]);
                },
                KW_APPENDFSYNC => {
                    let append_fsync = AppendFsync::try_from(args[i+1].as_str())
                        .expect("Unsupported value");
                    self.configs.set_appendfsync(append_fsync);
                }
                _ => {}
            };
            i += 2
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
