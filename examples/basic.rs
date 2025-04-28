//! The most basic usage

use libunftp::ServerBuilder;
use unftp_sbe_fatfs::Vfs;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let addr = "127.0.0.1:2121";

    let server = ServerBuilder::new(Box::new(move || Vfs::new("examples/my.img")))
        .greeting("Welcome to my FAT image over FTP")
        .passive_ports(50000..=65535)
        .build()
        .unwrap();

    println!("Starting FTP server on {}", addr);
    server.listen(addr).await.unwrap();
}
