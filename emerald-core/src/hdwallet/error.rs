//! # HDWallet Keystore files (UTC / JSON) module errors

use core;
use std::{error, fmt, io};

/// HDWallet Keystore file errors
#[derive(Debug)]
pub enum Error {
    /// An unsupported cipher
    HDWalletError(String),
    ///
    CommError(String),
}

impl From<core::Error> for Error {
    fn from(err: core::Error) -> Self {
        Error::HDWalletError(err.to_string())
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::HDWalletError(err.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::HDWalletError(ref str) => write!(f, "HD Wallet error: {}", str),
            Error::CommError(ref str) => write!(f, "Communication protocol error: {}", str),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "HD Wallet Keystore file error"
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            _ => None,
        }
    }
}
