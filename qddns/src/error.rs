use std::error::Error as StdError;
use std::fmt::{Display, Formatter};
use std::io;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for Error {}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::new(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
