use std::{fmt, io};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Invalid(String),
    Http(Box<dyn std::error::Error + Send + 'static>),
    Utf8(std::str::Utf8Error),

    #[doc(hidden)]
    __Nonexhaustive
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o: {}", e),
            Error::Http(e) => write!(f, "http: {}", e),
            Error::Invalid(s) => write!(f, "invalid: {}", s),
            Error::Utf8(e) => write!(f, "utf-8: {}", e),
            Error::__Nonexhaustive => f.write_str("__Nonexhaustive")
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Utf8(e) => Some(e),
            Error::Http(e) => Some(&**e),
            Error::Invalid(_)
            | Error::__Nonexhaustive => None
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Self {
        Error::Utf8(e)
    }
}
