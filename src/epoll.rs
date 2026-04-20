use std::os::fd::{AsRawFd, RawFd};
use std::io;

use libc;

// Event fired one then fd disarmed, prevent race condition in multi-thread
const READ_FLAGS: i32 = libc::EPOLLONESHOT | libc::EPOLLIN;
//const WRITE_FLAGS: i32 = libc::EPOLLONESHOT | libc::EPOLLOUT;

pub fn epoll_create() -> io::Result<RawFd> {
    // Create an epoll event queue
    // Return an fd to the queue

    let fd = syscall!(epoll_create1(0))?;
    
    // Prevent fd leak in case of fork
    if let Ok(flags) = syscall!(fcntl(fd, libc::F_GETFD)) {
        let _ = syscall!(fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC));
    }

    Ok(fd)
}

pub fn add_interest(
    epoll_fd: RawFd, 
    fd: RawFd, 
    mut event: libc::epoll_event) -> io::Result<()> {
    // Add an fd to epoll event queue
    // with notification criteria defined in &mut event struct

    // ADD (EPOLL_CTL_ADD) a fd to an epoll queue triggered on an event
    // defined in &mut event
    syscall!(epoll_ctl(epoll_fd, libc::EPOLL_CTL_ADD, fd, &mut event))?;
    Ok(())
}

pub fn modify_interest (
    epoll_fd: RawFd,
    fd: RawFd,
    mut event: libc::epoll_event) -> io::Result<()> {
    // Modify an already-added fd so it continues to get noti on

    syscall!(epoll_ctl(epoll_fd, libc::EPOLL_CTL_MOD, fd, &mut event))?;
    Ok(())
}

pub fn remove_interest(epoll_fd: RawFd, fd: RawFd) -> io::Result<()> {
    syscall!(epoll_ctl(epoll_fd, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut()))?;
    Ok(())
}

pub fn get_epoll_event_read(key: u64) -> libc::epoll_event {
    // Return an epoll event struct that
    // - get nofiy when events bit mask matched
    // - having u64 as identity key

    libc::epoll_event {
        events: READ_FLAGS as u32,
        u64: key, // fd of the fired event binded here
    }
}

