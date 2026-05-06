use libc;
use std::time::{SystemTime, UNIX_EPOCH};

#[macro_export]
macro_rules! syscall {
    ($fn: ident ( $($arg: expr),* $(,)* ) ) => {{
        let res = unsafe { libc::$fn($($arg, )*) };
        if res == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res)
        }
    }};
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap().as_millis() as u64
}
