use std::{fs::create_dir_all, path::Path, process};

use tokio::{fs::{remove_file, File}, io::{AsyncReadExt, AsyncWriteExt}};

use crate::tester::{self, Error, Result};

pub struct LocalFsClient {
    prefix: String,
    auto_increment: u32,
}

impl LocalFsClient {
    pub fn new() -> Self {
        let prefix = format!("/tmp/iotest_{}/", process::id());
        println!("INIT CLIENT");
        println!("  PREFIX:        {}", prefix);
        Self {
            prefix,
            auto_increment: 0,
        }
    }

    fn init(&self) {
        // Mkdir if the preifx directory is not existing.
        let prefix = Path::new(&self.prefix);
        if !prefix.exists() {
            create_dir_all(prefix).unwrap();
        }
    }
}

impl tester::TestClient for LocalFsClient {
    fn init(&self) {
        self.init()
    }

    fn gen_unique_key(&mut self) -> String {
        let result = format!("{}{}", self.prefix, self.auto_increment);
        self.auto_increment += 1;
        result
    }

    fn handler() -> impl tester::TestClientHandler {
        return LocalFsClientHandler{};
    }
}

pub struct LocalFsClientHandler;

impl tester::TestClientHandler for LocalFsClientHandler {
    async fn write(&self, key: &str, value: &str) -> Result<()> {
        let mut file = File::create(key).await
            .map_err(|err| Error::from_io_error(&format!("create {}", key), err))?;
        file.write_all(value.as_bytes()).await
            .map_err(|err| Error::from_io_error(&format!("write {}", key), err))?;
        Ok(())
    }

    async fn read(&self, key: &str) -> Result<String> {
        let mut file = File::open(key).await
            .map_err(|err| Error::from_io_error(&format!("open {}", key), err))?;
        let mut result = String::new();
        file.read_to_string(&mut result).await
            .map_err(|err| Error::from_io_error(&format!("read {}", key), err))?;
        Ok(result)
    }

    async fn delete(&self, key: &str) -> Result<()> {
        remove_file(key).await
            .map_err(|err| Error::from_io_error(&format!("delete {}", key), err))?;
        Ok(())
    }
}
