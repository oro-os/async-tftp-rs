#![allow(clippy::octal_escapes)]

//! Packet definitions.

use bytes::{BufMut, Bytes, BytesMut};
use std::convert::From;
use std::io;
use std::str;

use crate::error::Result;
use crate::parse::*;

pub const PACKET_DATA_HEADER_LEN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum PacketType {
    Rrq = 1,
    Wrq = 2,
    Data = 3,
    Ack = 4,
    Error = 5,
    OAck = 6,
}

/// TFTP protocol error. Should not be confused with `async_tftp::Error`.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum Error {
    #[error("client message: {0}")]
    Msg(String),
    #[error("unknown error")]
    UnknownError,
    #[error("file not found")]
    FileNotFound,
    #[error("permission denied")]
    PermissionDenied,
    #[error("disk full")]
    DiskFull,
    #[error("illegal operation")]
    IllegalOperation,
    #[error("unknown transfer ID")]
    UnknownTransferId,
    #[error("file already exists")]
    FileAlreadyExists,
    #[error("no such user")]
    NoSuchUser,
    #[error("options negotiation failed")]
    OptionsNegotiationFailed,
}

#[derive(Debug)]
pub enum Packet<'a> {
    Rrq(RwReq),
    Wrq(RwReq),
    Data(u16, &'a [u8]),
    Ack(u16),
    Error(Error),
    OAck(Opts),
}

#[derive(Debug, PartialEq)]
pub enum Mode {
    Netascii,
    Octet,
    Mail,
}

#[derive(Debug, PartialEq)]
pub struct RwReq {
    pub filename: String,
    pub mode: Mode,
    pub opts: Opts,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Opts {
    pub block_size: Option<u16>,
    pub timeout: Option<u8>,
    pub transfer_size: Option<u64>,
    pub window_size: Option<u64>,
}

impl PacketType {
    pub fn from_u16(n: u16) -> Option<PacketType> {
        match n {
            1 => Some(PacketType::Rrq),
            2 => Some(PacketType::Wrq),
            3 => Some(PacketType::Data),
            4 => Some(PacketType::Ack),
            5 => Some(PacketType::Error),
            6 => Some(PacketType::OAck),
            _ => None,
        }
    }
}

impl From<PacketType> for u16 {
    fn from(value: PacketType) -> Self {
        value as u16
    }
}

impl<'a> Packet<'a> {
    pub fn decode(data: &[u8]) -> Result<Packet> {
        parse_packet(data)
    }

    pub fn encode(&self, buf: &mut BytesMut) {
        match self {
            Packet::Rrq(req) => {
                buf.put_u16(PacketType::Rrq.into());
                buf.put_slice(req.filename.as_bytes());
                buf.put_u8(0);
                buf.put_slice(req.mode.to_str().as_bytes());
                buf.put_u8(0);
                req.opts.encode(buf);
            }
            Packet::Wrq(req) => {
                buf.put_u16(PacketType::Wrq.into());
                buf.put_slice(req.filename.as_bytes());
                buf.put_u8(0);
                buf.put_slice(req.mode.to_str().as_bytes());
                buf.put_u8(0);
                req.opts.encode(buf);
            }
            Packet::Data(block, data) => {
                buf.put_u16(PacketType::Data.into());
                buf.put_u16(*block);
                buf.put_slice(data);
            }
            Packet::Ack(block) => {
                buf.put_u16(PacketType::Ack.into());
                buf.put_u16(*block);
            }
            Packet::Error(error) => {
                buf.put_u16(PacketType::Error.into());
                buf.put_u16(error.code());
                buf.put_slice(error.msg().as_bytes());
                buf.put_u8(0);
            }
            Packet::OAck(opts) => {
                buf.put_u16(PacketType::OAck.into());
                opts.encode(buf);
            }
        }
    }

    pub fn encode_data_head(block_id: u16, buf: &mut BytesMut) {
        buf.put_u16(PacketType::Data.into());
        buf.put_u16(block_id);
    }

    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::new();
        self.encode(&mut buf);
        buf.freeze()
    }
}

impl Opts {
    fn encode(&self, buf: &mut BytesMut) {
        if let Some(block_size) = self.block_size {
            buf.put_slice(&b"blksize\0"[..]);
            buf.put_slice(block_size.to_string().as_bytes());
            buf.put_u8(0);
        }

        if let Some(timeout) = self.timeout {
            buf.put_slice(&b"timeout\0"[..]);
            buf.put_slice(timeout.to_string().as_bytes());
            buf.put_u8(0);
        }

        if let Some(window_size) = self.window_size {
            buf.put_slice(&b"windowsize\0"[..]);
            buf.put_slice(window_size.to_string().as_bytes());
            buf.put_u8(0);
        }

        if let Some(transfer_size) = self.transfer_size {
            buf.put_slice(&b"tsize\0"[..]);
            buf.put_slice(transfer_size.to_string().as_bytes());
            buf.put_u8(0);
        }
    }
}

impl Mode {
    pub fn to_str(&self) -> &'static str {
        match self {
            Mode::Netascii => "netascii",
            Mode::Octet => "octet",
            Mode::Mail => "mail",
        }
    }
}

impl Error {
    pub fn from_code(code: u16, msg: Option<&str>) -> Self {
        #[allow(clippy::wildcard_in_or_patterns)]
        match code {
            1 => Error::FileNotFound,
            2 => Error::PermissionDenied,
            3 => Error::DiskFull,
            4 => Error::IllegalOperation,
            5 => Error::UnknownTransferId,
            6 => Error::FileAlreadyExists,
            7 => Error::NoSuchUser,
            8 => Error::OptionsNegotiationFailed,
            0 | _ => match msg {
                Some(msg) => Error::Msg(msg.to_string()),
                None => Error::UnknownError,
            },
        }
    }

    pub fn code(&self) -> u16 {
        match self {
            Error::Msg(..) => 0,
            Error::UnknownError => 0,
            Error::FileNotFound => 1,
            Error::PermissionDenied => 2,
            Error::DiskFull => 3,
            Error::IllegalOperation => 4,
            Error::UnknownTransferId => 5,
            Error::FileAlreadyExists => 6,
            Error::NoSuchUser => 7,
            Error::OptionsNegotiationFailed => 8,
        }
    }

    pub fn msg(&self) -> &str {
        match self {
            Error::Msg(msg) => msg,
            Error::UnknownError => "Unknown error",
            Error::FileNotFound => "File not found",
            Error::PermissionDenied => "Permission denied",
            Error::DiskFull => "Disk is full",
            Error::IllegalOperation => "Illegal operation",
            Error::UnknownTransferId => "Unknown transfer ID",
            Error::FileAlreadyExists => "File already exists",
            Error::NoSuchUser => "No such user",
            Error::OptionsNegotiationFailed => "Options negotiation failed",
        }
    }
}

impl From<Error> for Packet<'_> {
    fn from(inner: Error) -> Self {
        Packet::Error(inner)
    }
}

impl From<io::Error> for Error {
    fn from(io_err: io::Error) -> Self {
        match io_err.kind() {
            io::ErrorKind::NotFound => Error::FileNotFound,
            io::ErrorKind::PermissionDenied => Error::PermissionDenied,
            io::ErrorKind::WriteZero => Error::DiskFull,
            io::ErrorKind::AlreadyExists => Error::FileAlreadyExists,
            _ => match io_err.raw_os_error() {
                Some(rc) => Error::Msg(format!("IO error: {}", rc)),
                None => Error::UnknownError,
            },
        }
    }
}

impl From<crate::Error> for Error {
    fn from(err: crate::Error) -> Self {
        match err {
            crate::Error::Packet(e) => e,
            crate::Error::Io(e) => e.into(),
            crate::Error::InvalidPacket => Error::IllegalOperation,
            crate::Error::MaxSendRetriesReached(..) => {
                Error::Msg("Max retries reached".to_string())
            }
            _ => Error::UnknownError,
        }
    }
}
