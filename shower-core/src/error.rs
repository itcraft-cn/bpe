use std::{
    error::Error,
    fmt::{Display, Formatter},
};

#[derive(Debug)]
pub(crate) struct ActionError {
    msg: String,
}
impl ActionError {
    pub(crate) fn new(msg: &str) -> Self {
        Self {
            msg: String::from(msg),
        }
    }
    pub(crate) fn new_string(msg: String) -> Self {
        Self { msg: msg.clone() }
    }
}
impl Display for ActionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error[{}] occurred when generating action", self.msg)
    }
}
impl Error for ActionError {
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
