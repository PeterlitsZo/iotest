use std::{future::Future, io, result};

#[derive(Debug)]
pub struct Error {
    pub msg: String,
}

impl Error {
    pub fn from_io_error(prefix: &str, err: io::Error) -> Self {
        Self {
            msg: format!("{}: {}", prefix, err),
        }
    }
}

pub type Result<T> = result::Result<T, Error>;

/// TestClient is used to talk with a storage.
pub trait TestClient: Send + Sync {
    /// Generate an unique key to write / read / delete object.
    fn gen_unique_key(&mut self) -> String;

    /// Init the client.
    fn init(&self);

    /// Get the handler.
    fn handler() -> impl TestClientHandler;
}

pub trait TestClientHandler: Send {
    /// Write a object.
    fn write(&self, key: &str, value: &str) -> impl Future<Output = Result<()>> + Send;

    /// Read a object.
    fn read(&self, key: &str) -> impl Future<Output = Result<String>> + Send;

    /// Delete a object.
    fn delete(&self, key: &str) -> impl Future<Output = Result<()>> + Send;
}