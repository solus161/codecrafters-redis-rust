// let password = values.pop_front()
//             .ok_or(CustomError::MissingArgument(msg_password.to_string()))?
//             .get_str()
//             .ok_or(CustomError::MissingArgument(msg_password.to_string()))?;
//
macro_rules! resp_arg {
    ($values:expr, $err_msg:expr, String) => {
         $values.pop_front()
            .ok_or(CustomError::MissingArgument($err_msg.to_string()))?
            .get_str()
            .ok_or(CustomError::MissingArgument($err_msg.to_string()))?
    };
    ($values:expr, $err_msg:expr, i64) => {
         $values.pop_front()
            .ok_or(CustomError::MissingArgument($err_msg.to_string()))?
            .get_int()
            .ok_or(CustomError::MissingArgument($err_msg.to_string()))?
    };
}

pub(crate) use resp_arg;
