use std::{
    error::Error,
    fmt::{Display, Formatter},
};

#[derive(Debug)]
pub(crate) struct ParseSqlError {
    msg: String,
}
impl ParseSqlError {
    pub(crate) fn new(msg: String) -> Self {
        Self { msg: msg.clone() }
    }
}
impl Display for ParseSqlError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error[{}] occurred", self.msg)
    }
}
impl Error for ParseSqlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "error occurred"
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}
