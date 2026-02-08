//! A storage backend for [libunftp](https://github.com/bolcom/libunftp) that provides read-only access to FAT filesystem images.
//!
//! This crate implements a storage backend for the libunftp FTP server library, allowing you to serve files from FAT filesystem images (`.img` files) over FTP.
//!
//! # Example
//!
//! ```no_run
//! use libunftp::ServerBuilder;
//! use unftp_sbe_fatfs::Vfs;
//!
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() {
//!     let addr = "127.0.0.1:2121";
//!
//!     let server = ServerBuilder::new(Box::new(move || Vfs::new("examples/my.img")))
//!         .greeting("Welcome to my FAT image over FTP")
//!         .passive_ports(50000..=65535)
//!         .build()
//!         .unwrap();
//!
//!     println!("Starting FTP server on {}", addr);
//!     server.listen(addr).await.unwrap();
//! }
//! ```
//!
//! # Connecting with lftp
//!
//! Once your FTP server is running, you can connect to it using an FTP client like [lftp](https://lftp.yar.ru/), a sophisticated file transfer program that supports multiple protocols including FTP:
//!
//! ```bash
//! # Connect to the server
//! lftp ftp://localhost:2121
//!
//! # List files in the current directory
//! ls
//!
//! # Change to a subdirectory
//! cd subdir
//!
//! # Download a file
//! get filename.txt
//!
//! # Mirror a directory (download all files)
//! mirror directory_name
//!
//! # Exit lftp
//! exit
//! ```
//!
//! # Limitations
//!
//! - Read-only access (no file uploads, deletions, or modifications)
//! - No support for symbolic links

use async_trait::async_trait;
use fatfs::{DateTime, DirEntry, FileSystem, FsOptions};
use libunftp::{
    auth::UserDetail,
    storage::{Error, ErrorKind, Fileinfo, Metadata, Result, StorageBackend},
};
use std::{
    fmt::Debug,
    fs::File,
    io::{Cursor, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::Duration,
    time::SystemTime,
};

/// A virtual file system that provides read-only access to FAT filesystem images.
///
/// This struct implements the `StorageBackend` trait from libunftp, allowing it to be used
/// as a storage backend for an FTP server. It provides read-only access to the contents
/// of a FAT filesystem image file.
///
/// # Example
///
/// ```rust
/// use unftp_sbe_fatfs::Vfs;
///
/// let vfs = Vfs::new("path/to/fat/image.img");
/// ```
#[derive(Debug, Clone)]
pub struct Vfs {
    img_path: PathBuf,
}

impl Vfs {
    /// Creates a new virtual file system that provides access to the FAT image file
    /// at the given path.
    ///
    /// # Arguments
    ///
    /// * `img_path` - The path to the FAT filesystem image file
    ///
    /// # Example
    ///
    /// ```rust
    /// use unftp_sbe_fatfs::Vfs;
    ///
    /// let vfs = Vfs::new("path/to/fat/image.img");
    /// ```
    pub fn new<P: AsRef<Path>>(img_path: P) -> Self {
        Self {
            img_path: img_path.as_ref().to_path_buf(),
        }
    }

    /// Opens the FAT filesystem image and returns a `FileSystem` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the image file cannot be opened or if it's not a valid
    /// FAT filesystem image.
    fn open_fs(&self) -> Result<FileSystem<File>> {
        let f = File::open(&self.img_path).map_err(Error::from)?;
        let fs = FileSystem::new(f, FsOptions::new()).map_err(Error::from)?;
        Ok(fs)
    }

    /// Finds a file or directory entry in the FAT filesystem.
    ///
    /// # Arguments
    ///
    /// * `fs` - The opened filesystem instance
    /// * `ftp_path` - The path to find, relative to the root of the filesystem
    ///
    /// # Errors
    ///
    /// Returns an error if the path doesn't exist or if there's an error accessing
    /// the filesystem.
    fn find<'a, P: AsRef<Path>>(
        &self,
        fs: &'a FileSystem<File>,
        ftp_path: P,
    ) -> Result<DirEntry<'a, File>> {
        let path = self.normalize_path(ftp_path.as_ref());

        // Start from the root directory
        let root_dir = fs.root_dir();

        // If path is just the root, handle specially
        if path == Path::new("/") || path.as_os_str().is_empty() {
            // Return a special case for root, or error depending on your needs
            return Err(ErrorKind::FileNameNotAllowedError.into());
            // Alternatively, you might have a way to represent root as a DirEntry
        }

        // Strip leading slash if present
        let path_str = path.to_string_lossy();
        let path_str = path_str.trim_start_matches('/');

        // Split the path into components
        let components: Vec<&str> = path_str.split('/').collect();

        // Navigate through each component
        let mut current_dir = root_dir;
        let mut current_entry: Option<DirEntry<File>> = None;

        // Handle all components except the last one (which may be a file)
        for (i, component) in components.iter().enumerate() {
            if component.is_empty() {
                continue;
            }

            // Iterate through directory entries to find the component
            let mut found = false;
            for entry_result in current_dir.iter() {
                let entry = entry_result.map_err(|_| {
                    let e: Error = ErrorKind::PermanentFileNotAvailable.into();
                    e
                })?;

                // Compare the entry name with the current component (case-insensitive for FAT)
                if entry.file_name().eq_ignore_ascii_case(component) {
                    // If this is the last component, we've found our entry
                    if i == components.len() - 1 {
                        current_entry = Some(entry);
                        found = true;
                        break;
                    }

                    // Otherwise, ensure this component is a directory and continue navigating
                    if entry.is_dir() {
                        current_dir = entry.to_dir();
                        found = true;
                        break;
                    } else {
                        // Found a file but expected a directory
                        return Err(ErrorKind::FileNameNotAllowedError.into());
                    }
                }
            }

            if !found {
                return Err(ErrorKind::PermanentFileNotAvailable.into());
            }
        }

        current_entry.ok_or(ErrorKind::PermanentFileNotAvailable.into())
    }

    /// Normalizes an FTP path to a consistent format.
    ///
    /// This function handles path components like '..' and '.' to produce a
    /// canonical path representation.
    fn normalize_path(&self, path: &Path) -> PathBuf {
        // Convert to a canonical form, resolving '..' and '.'
        // This is a simplified version - you might need more robust handling
        let mut result = PathBuf::new();

        for component in path.components() {
            match component {
                std::path::Component::ParentDir => {
                    // Go up one level if possible
                    if !result.as_os_str().is_empty() {
                        result.pop();
                    }
                }
                std::path::Component::Normal(name) => result.push(name),
                std::path::Component::CurDir => {} // Skip '.' components
                _ => {}                            // Skip other components
            }
        }

        result
    }
}

#[async_trait]
impl<User: UserDetail> StorageBackend<User> for Vfs {
    type Metadata = Meta;

    async fn metadata<P: AsRef<Path> + Send + Debug>(
        &self,
        _user: &User,
        path: P,
    ) -> Result<Self::Metadata> {
        let fs = self.open_fs()?;

        let e = self.find(&fs, path)?;

        Ok(Meta {
            is_dir: e.is_dir(),
            len: e.len(),
            modified: e.modified(),
        })
    }

    async fn list<P: AsRef<Path> + Send + Debug>(
        &self,
        _user: &User,
        path: P,
    ) -> Result<Vec<Fileinfo<PathBuf, Self::Metadata>>>
    where
        <Self as StorageBackend<User>>::Metadata: Metadata,
    {
        let mut entries = Vec::new();
        let fs = self.open_fs()?;
        let dir = if path.as_ref().to_str().unwrap().eq("/") {
            fs.root_dir()
        } else {
            let entry = self.find(&fs, path)?;
            if entry.is_file() {
                return Err(Error::from(ErrorKind::FileNameNotAllowedError));
            }
            entry.to_dir()
        };

        for sub_result in dir.iter() {
            let sub = sub_result.map_err(|_| {
                let e: Error = ErrorKind::PermanentFileNotAvailable.into();
                e
            })?;
            entries.push(Fileinfo {
                path: sub.file_name().into(),
                metadata: Meta {
                    is_dir: sub.is_dir(),
                    len: sub.len(),
                    modified: sub.modified(),
                },
            })
        }

        Ok(entries)
    }

    async fn get<P: AsRef<Path> + Send + Debug>(
        &self,
        _user: &User,
        path: P,
        start_pos: u64,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Sync + Unpin>> {
        let fs = self.open_fs()?;
        let entry = self.find(&fs, path)?;

        if entry.is_dir() {
            return Err(ErrorKind::FileNameNotAllowedError.into());
        }

        let mut file = entry.to_file();

        // Seek to the starting position
        file.seek(SeekFrom::Start(start_pos))
            .map_err(|_| ErrorKind::PermanentFileNotAvailable)?;

        // Read entire contents into a Vec<u8>
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).map_err(|e| {
            Error::new(
                ErrorKind::PermanentFileNotAvailable,
                format!("read error: {e}"),
            )
        })?;

        // Return a cursor over the buffer to provide async access
        let cursor = Cursor::new(buf);
        Ok(Box::new(cursor))
    }

    async fn put<
        P: AsRef<Path> + Send + Debug,
        R: tokio::io::AsyncRead + Send + Sync + Unpin + 'static,
    >(
        &self,
        _user: &User,
        _input: R,
        _path: P,
        _start_pos: u64,
    ) -> Result<u64> {
        Err(Error::from(ErrorKind::PermissionDenied))
    }

    async fn del<P: AsRef<Path> + Send + Debug>(&self, _user: &User, _path: P) -> Result<()> {
        Err(Error::from(ErrorKind::PermissionDenied))
    }

    async fn mkd<P: AsRef<Path> + Send + Debug>(&self, _user: &User, _path: P) -> Result<()> {
        Err(Error::from(ErrorKind::PermissionDenied))
    }

    async fn rename<P: AsRef<Path> + Send + Debug>(
        &self,
        _user: &User,
        _from: P,
        _to: P,
    ) -> Result<()> {
        Err(Error::from(ErrorKind::PermissionDenied))
    }

    async fn rmd<P: AsRef<Path> + Send + Debug>(&self, _user: &User, _path: P) -> Result<()> {
        Err(Error::from(ErrorKind::PermissionDenied))
    }

    async fn cwd<P: AsRef<Path> + Send + Debug>(&self, _user: &User, path: P) -> Result<()> {
        let fs = self.open_fs()?;
        if path.as_ref().to_str().unwrap().eq("/") {
            return Ok(());
        }

        let entry = self.find(&fs, path)?;
        if entry.is_file() {
            return Err(Error::from(ErrorKind::FileNameNotAllowedError));
        }
        Ok(())
    }
}

/// Metadata for files and directories in the FAT filesystem.
///
/// This struct implements the `Metadata` trait from libunftp and provides
/// information about files and directories in the FAT filesystem.
#[derive(Debug, Clone)]
pub struct Meta {
    is_dir: bool,
    len: u64,
    modified: DateTime,
}

impl Metadata for Meta {
    fn len(&self) -> u64 {
        self.len
    }

    fn is_dir(&self) -> bool {
        self.is_dir
    }

    fn is_file(&self) -> bool {
        !self.is_dir
    }

    fn is_symlink(&self) -> bool {
        false
    }

    fn modified(&self) -> Result<SystemTime> {
        let dt = &self.modified;

        // FAT timestamps start at 1980-01-01 00:00:00
        let fat_epoch = SystemTime::UNIX_EPOCH + Duration::from_secs(315532800); // seconds from 1970 to 1980

        // Simple sanity check
        if dt.date.year < 1980
            || dt.date.month == 0
            || dt.date.month > 12
            || dt.date.day == 0
            || dt.date.day > 31
        {
            return Err(ErrorKind::PermanentFileNotAvailable.into());
        }

        // Days since 1980-01-01
        let days = days_since_1980(dt.date.year, dt.date.month, dt.date.day)
            .ok_or(ErrorKind::PermanentFileNotAvailable)?;

        let seconds = (days as u64) * 86400
            + (dt.time.hour as u64) * 3600
            + (dt.time.min as u64) * 60
            + (dt.time.sec as u64);

        Ok(fat_epoch + Duration::from_secs(seconds))
    }

    fn gid(&self) -> u32 {
        0
    }

    fn uid(&self) -> u32 {
        0
    }
}

// Helper to compute number of days since 1980-01-01
fn days_since_1980(year: u16, month: u16, day: u16) -> Option<u32> {
    // Days in each month, not accounting for leap years yet
    const DAYS_IN_MONTH: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    // Basic validation
    if !(1..=12).contains(&month) || day < 1 {
        return None;
    }

    let mut days = 0u32;

    // Years
    for y in 1980..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }

    // Months
    for m in 1..month {
        days += DAYS_IN_MONTH[(m - 1) as usize];
        if m == 2 && is_leap_year(year) {
            days += 1;
        }
    }

    // Days
    days += (day as u32) - 1;

    Some(days)
}

fn is_leap_year(year: u16) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}
