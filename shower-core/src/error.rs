use std::{
    error::Error,
    fmt::{Display, Formatter},
};

#[derive(Debug)]
pub(crate) struct MapperError {
    msg: String,
}
impl MapperError {
    pub(crate) fn new(msg: String) -> Self {
        Self { msg: msg.clone() }
    }
}
impl Display for MapperError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error[{}] occurred when generating action", self.msg)
    }
}
impl Error for MapperError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }

    fn description(&self) -> &str {
        "error occurred when generating action"
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}
