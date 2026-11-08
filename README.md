# unftp-sbe-fatfs

A storage backend for [libunftp](https://github.com/Cloudinvoke/libunftp) that provides read-only access to FAT filesystem images.

While feeling nostalgic I implemented this storage backend for the libunftp FTP server library, allowing you to serve
files from FAT filesystem images (`.img` files) over FTP. The implementation is read-only, meaning clients can list
directories and download files, but cannot modify the filesystem. Supporting write operations wouldn't be out of the
question but perhaps less useful. For now this is it.

## Features

- Read-only access to FAT filesystem images
- Directory listing
- File metadata (size, modification time)
- Position-based file reading

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
unftp-sbe-fatfs = "0.2.1"
libunftp = "0.24.0"
tokio = { version = "1", features = ["full"] }
```

### Basic Example

Here's a simple example of how to use this crate to serve a FAT image over FTP:

```rust
use libunftp::ServerBuilder;
use unftp_sbe_fatfs::Vfs;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let addr = "127.0.0.1:2121";

    let server = ServerBuilder::new(Box::new(move || Vfs::new("/path/to/image.img")))
        .greeting("Welcome to my FAT image over FTP")
        .passive_ports(50000..=65535)
        .build()
        .unwrap();

    println!("Starting FTP server on {}", addr);
    server.listen(addr).await.unwrap();
}
```

Replace `/path/to/image.img` with the path to a FAT image. For a runnable repository example,
see [`examples/basic.rs`](https://github.com/hannesdejager/unftp-sbe-fatfs/blob/master/examples/basic.rs).

### Connecting with an FTP client

Once your FTP server is running, you can connect to it using an FTP client like [lftp](https://lftp.yar.ru/), a sophisticated file transfer program that supports multiple protocols including FTP:

```bash
# Connect to the server
lftp ftp://localhost:2121

# List files in the current directory
ls

# Change to a subdirectory
cd subdir

# Download a file
get filename.txt

# Mirror a directory (download all files)
mirror directory_name

# Exit lftp
exit
```

## Limitations

- Read-only access (no file uploads, deletions, or modifications)
- Currently only supports FAT filesystem images
- No support for symbolic links
- Image reads use synchronous file I/O inside the backend's async methods.

## License

This project is licensed under the Apache-2.0 License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request, especially if you want to implement the write
operations.
