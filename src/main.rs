use tokio;

use client::localfs::LocalFsClient;
use tester::Tester;

pub mod tester;
mod client;

#[tokio::main]
async fn main() {
    let localfs = LocalFsClient::new();
    let mut tester = Tester::new(localfs, 16 * 1024 * 1024 /* 16MiB */);
    tester.test().await;
}
