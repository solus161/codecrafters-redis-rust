use std::{num::{ParseFloatError, ParseIntError}, string::FromUtf8Error};

use crate::resp::RespType;


pub const ERR_RDB_PARENT: &str = "RDB parent folder path not exists";
pub const ERR_RDB_CREATE: &str = "Error creating RDB file";
pub const ERR_HOST_STATS_NOT_INITIATED: &str = "Host stats not initiated at startup";
pub const ERR_MASTER_STATS_NOT_INITIATED: &str = "Master stats not initiated at startup";
pub const ERR_MASTER_STATS_HOST_NOT_SET: &str = "Master's host is not set";
pub const ERR_MASTER_STATS_PORT_NOT_SET: &str = "Master's port is not set";
pub const ERR_CREATING_EPOLL: &str = "Error creating epoll queue";
pub const ERR_WATCH_IN_MULTI: &str = "ERR WATCH inside MULTI is not allowed";

#[derive(Debug)]
pub enum CustomError {
    PathNotExists(String),
    InvalidArgument(String),
    MissingArgument(String),
    ParseError(String),
    NoCmdError(String),
    UnsupportedCmdStructure(String),
    UnsupportedCmd(String),
    UnprocessableError(String),
    InternalError(String),
    FileReadingError,
    RDBParsingError,
    WrongUsernamePassword(String),
}

impl CustomError {
    pub fn into_resp(self) -> RespType {
        let msg = match self {
            Self::PathNotExists(s) |
            Self::InvalidArgument(s) |
            Self::MissingArgument(s) |
            Self::ParseError(s) |
            Self::NoCmdError(s) |
            Self::UnsupportedCmdStructure(s) |
            Self::UnsupportedCmd(s) |
            Self::UnprocessableError(s) |
            Self::InternalError(s) |
            Self::WrongUsernamePassword(s)=> {
                s
            },
            Self::FileReadingError => "Error reading file".into(),
            Self::RDBParsingError => "Error parsing RDB file".into(), 
        };
        RespType::Error(Some(msg)) 
    }

    pub fn into_error_bytes(self) -> Vec<u8> {
        self.into_resp().serialize()
    }

    pub fn err_path_not_exists(path: &str) -> Self {
        Self::PathNotExists(format!("Path not exists: {}", path))
    }

    pub fn err_unsupported_cmd_struct(arg: &str) -> Self {
        Self::UnsupportedCmdStructure(arg.to_string())
    }
}

impl From<CustomError> for Vec<u8> {
    fn from(e: CustomError) -> Self {
        e.into_error_bytes()
    }
}

impl From<ParseIntError> for CustomError {
    fn from(e: ParseIntError) -> Self {
        Self::ParseError(e.to_string()) 
    }
}

impl From<ParseFloatError> for CustomError {
    fn from(e: ParseFloatError) -> Self {
        Self::ParseError(e.to_string()) 
    }
}

impl From<FromUtf8Error> for CustomError {
    fn from(_: FromUtf8Error) -> Self {
        Self::ParseError("Error parsing UTF8".to_string()) 
    }
}

impl From<std::io::Error> for CustomError {
    fn from(_: std::io::Error) -> Self {
        Self::FileReadingError        
    }
}
