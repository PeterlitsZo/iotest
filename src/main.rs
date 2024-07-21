use tokio;

use client::localfs::LocalFsClient;
use tester::Tester;

pub mod tester;
mod client;

#[tokio::main]
async fn main() {
    let localfs = LocalFsClient::new();
    let mut tester = Tester::new(localfs);
    tester.test().await;
}
