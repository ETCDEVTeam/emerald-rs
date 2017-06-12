//! # Node managment module errors

use std::convert::From;
use std::fmt;
use std::io;
use subprocess::PopenError;

///
pub enum Error {
    /// Invalid chain type
    InvalidChain(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::InvalidChain(ref str) => write!(f, "Invalid chain type: {}", str),
        }
    }
}

impl From<PopenError> for Error {
    fn from(e: PopenError) -> Self {
        unimplemented!()
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        unimplemented!()
    }
}