use thiserror::Error;
pub mod db;

pub use db::Db as MemcachedDb;

mod protocol;

#[derive(Error, Debug)]
pub enum Error {
    #[error("ProtocolError")]
    ProtocolError,
    #[error("IOError: {0}")]
    IOError(#[from] std::io::Error),
}

pub type MemcachedError = Error;