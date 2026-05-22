use std::fmt::Display;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{OnceLock, RwLock};
use std::rc::Rc;

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

pub struct ReplStats {
    // This is used to store replication stats
    // of the host and its master if any
    pub host: Option<String>,
    pub port: Option<u16>,
    pub id: RwLock<String>,
    pub offset: AtomicOffset, // Offset off current server (slave)
}

impl ReplStats {
    pub fn new(host: Option<String>, port: Option<u16>) -> Self {
        Self {
            host,
            port,
            id: RwLock::new("?".to_string()),
            offset: AtomicOffset::new(-1)}
    }

    fn parse_replicaof(s: Option<String>) -> Option<Self> {
        if let Some(v) = s {
            let mut splitted = v.split(" ");
            let host = splitted.next().expect("Invalid replication host").to_string();
            let port: u16 = splitted.next().expect("Invalid replication port")
                .parse().expect("Invalid replication port");

            Some(Self {
                host: Some(host),
                port: Some(port),
                id: RwLock::new("?".to_string()),
                offset: AtomicOffset::new(-1)})
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

}

pub struct AppStates {
    pub host_stats: Option<ReplStats>,
    pub master_stats: Option<ReplStats>,
}

impl AppStates {
    pub fn init(args: Vec<String>) {
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
        
        let _ = APP_STATES.set(Self{ host_stats: Some(host_resp), master_stats: master_repl});
    }

    pub fn get() -> &'static AppStates {
        APP_STATES.get().expect("Config not initialized")
    }

    pub fn is_slave(&self) -> bool {
        match self.master_stats {
            Some(_) => true,
            None => false
        }
    }
}

static APP_STATES: OnceLock<AppStates> = OnceLock::new();
