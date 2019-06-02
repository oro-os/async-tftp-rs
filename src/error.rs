use failure::Fail;
pub(crate) use std::result::Result as StdResult;

pub(crate) type Result<T> = StdResult<T, Error>;

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Invalid mode")]
    InvalidMode,

    #[fail(display = "Invalid packet")]
    InvalidPacket,

    #[fail(display = "IO Error: {}", _0)]
    Io(std::io::Error),
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Error {
        Error::Io(error)
    }
}
