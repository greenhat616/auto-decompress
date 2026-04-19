//! Rust bindings for bit7z - a C++ wrapper for 7-zip
//!
//! This crate provides safe Rust bindings to read and extract archives using the bit7z library.
//!
//! # Features
//!
//! - Read encrypted and multi-volume archives
//! - List archive contents with metadata
//! - Extract specific files by index or pattern
//! - Support for many archive formats (7z, zip, rar, tar, etc.)

mod ffi;

use cxx::UniquePtr;
use std::path::Path;
use thiserror::Error;

pub use ffi::ArchiveFormat;

/// Error types for bit7z operations
#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to load 7z library: {0}")]
    LibraryLoad(String),

    #[error("Failed to open archive: {0}")]
    ArchiveOpen(String),

    #[error("Invalid password")]
    InvalidPassword,

    #[error("Extraction failed: {0}")]
    Extraction(String),

    #[error("Invalid index: {0}")]
    InvalidIndex(u32),

    #[error("Invalid format")]
    InvalidFormat,

    #[error("I/O error: {0}")]
    Io(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<ffi::ErrorCode> for Error {
    fn from(code: ffi::ErrorCode) -> Self {
        match code {
            ffi::ErrorCode::Ok => unreachable!(),
            ffi::ErrorCode::LibraryLoadFailed => Error::LibraryLoad(String::new()),
            ffi::ErrorCode::ArchiveOpenFailed => Error::ArchiveOpen(String::new()),
            ffi::ErrorCode::InvalidPassword => Error::InvalidPassword,
            ffi::ErrorCode::ExtractionFailed => Error::Extraction(String::new()),
            ffi::ErrorCode::InvalidIndex => Error::InvalidIndex(0),
            ffi::ErrorCode::InvalidFormat => Error::InvalidFormat,
            ffi::ErrorCode::IoError => Error::Io(String::new()),
            ffi::ErrorCode::UnknownError => Error::Unknown(String::new()),
            _ => Error::Unknown(String::new()),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Information about a single item in an archive
#[derive(Debug, Clone)]
pub struct ArchiveItem {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub packed_size: u64,
    pub crc: u64,
    pub creation_time: Option<i64>,
    pub modification_time: Option<i64>,
    pub access_time: Option<i64>,
    pub attributes: u32,
    pub index: u32,
    pub is_dir: bool,
    pub is_encrypted: bool,
}

impl From<ffi::ArchiveItemInfo> for ArchiveItem {
    fn from(info: ffi::ArchiveItemInfo) -> Self {
        Self {
            path: info.path,
            name: info.name,
            size: info.size,
            packed_size: info.packed_size,
            crc: info.crc,
            creation_time: if info.creation_time >= 0 {
                Some(info.creation_time)
            } else {
                None
            },
            modification_time: if info.modification_time >= 0 {
                Some(info.modification_time)
            } else {
                None
            },
            access_time: if info.access_time >= 0 {
                Some(info.access_time)
            } else {
                None
            },
            attributes: info.attributes,
            index: info.index,
            is_dir: info.is_dir,
            is_encrypted: info.is_encrypted,
        }
    }
}

/// Metadata about an archive
#[derive(Debug, Clone)]
pub struct ArchiveMetadata {
    pub size: u64,
    pub packed_size: u64,
    pub items_count: u32,
    pub folders_count: u32,
    pub files_count: u32,
    pub volumes_count: u32,
    pub is_encrypted: bool,
    pub has_encrypted_items: bool,
    pub is_multi_volume: bool,
    pub is_solid: bool,
}

impl From<ffi::ArchiveInfo> for ArchiveMetadata {
    fn from(info: ffi::ArchiveInfo) -> Self {
        Self {
            size: info.size,
            packed_size: info.packed_size,
            items_count: info.items_count,
            folders_count: info.folders_count,
            files_count: info.files_count,
            volumes_count: info.volumes_count,
            is_encrypted: info.is_encrypted,
            has_encrypted_items: info.has_encrypted_items,
            is_multi_volume: info.is_multi_volume,
            is_solid: info.is_solid,
        }
    }
}

/// Handle to the 7z library. Must be kept alive while using archives.
pub struct Library {
    inner: UniquePtr<ffi::Bit7zLibrary>,
}

impl Library {
    /// Load the 7z library from the specified DLL path.
    ///
    /// On Windows, this is typically "7z.dll".
    /// On Linux, this is typically "/usr/lib/p7zip/7z.so".
    pub fn new(dll_path: impl AsRef<Path>) -> Result<Self> {
        let path_str = dll_path.as_ref().to_string_lossy().to_string();
        let lib = ffi::new_library(&path_str);

        if !lib.is_valid() {
            return Err(Error::LibraryLoad(lib.get_error()));
        }

        Ok(Self { inner: lib })
    }

    /// Load the 7z library from the default location.
    pub fn load_default() -> Result<Self> {
        #[cfg(windows)]
        const DEFAULT_PATH: &str = "7z.dll";
        #[cfg(not(windows))]
        const DEFAULT_PATH: &str = "/usr/lib/p7zip/7z.so";

        Self::new(DEFAULT_PATH)
    }
}

/// Reader for archive files. Provides metadata and extraction capabilities.
pub struct ArchiveReader {
    inner: UniquePtr<ffi::ArchiveReader>,
}

impl ArchiveReader {
    /// Open an archive file.
    ///
    /// # Arguments
    /// * `lib` - The loaded 7z library
    /// * `path` - Path to the archive file
    /// * `format` - Archive format (use `ArchiveFormat::Auto` for auto-detection)
    /// * `password` - Password for encrypted archives (empty string if none)
    pub fn open(
        lib: &Library,
        path: impl AsRef<Path>,
        format: ArchiveFormat,
        password: &str,
    ) -> Result<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let reader = ffi::new_archive_reader(&lib.inner, &path_str, format, password);

        if !reader.is_valid() {
            return Err(Error::ArchiveOpen(reader.get_error()));
        }

        Ok(Self { inner: reader })
    }

    /// Open a multi-volume archive (pass the first volume path).
    pub fn open_multi_volume(
        lib: &Library,
        first_volume_path: impl AsRef<Path>,
        format: ArchiveFormat,
        password: &str,
    ) -> Result<Self> {
        Self::open(lib, first_volume_path, format, password)
    }

    /// Get archive metadata.
    pub fn metadata(&self) -> ArchiveMetadata {
        self.inner.get_archive_info().into()
    }

    /// List all items in the archive.
    pub fn items(&self) -> Vec<ArchiveItem> {
        self.inner
            .get_items()
            .into_iter()
            .map(|i| i.into())
            .collect()
    }

    /// Get information about a specific item by index.
    pub fn item(&self, index: u32) -> Option<ArchiveItem> {
        let item = self.inner.get_item(index);
        if item.path.is_empty() && item.name.is_empty() {
            None
        } else {
            Some(item.into())
        }
    }

    /// Check if the archive is encrypted.
    pub fn is_encrypted(&self) -> bool {
        self.inner.is_encrypted()
    }

    /// Check if the archive header is encrypted.
    pub fn is_header_encrypted(&self) -> bool {
        self.inner.is_header_encrypted()
    }

    /// Extract a single item by index to a directory.
    pub fn extract_to_dir(&self, index: u32, out_dir: impl AsRef<Path>) -> Result<()> {
        let out_str = out_dir.as_ref().to_string_lossy().to_string();
        let result = self.inner.extract_to_dir(index, &out_str);
        self.check_result(result)
    }

    /// Extract a single item by index to a memory buffer.
    pub fn extract_to_buffer(&self, index: u32) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        let result = self.inner.extract_to_buffer(index, &mut buffer);
        self.check_result(result)?;
        Ok(buffer)
    }

    /// Extract multiple items by indices to a directory.
    pub fn extract_items(&self, indices: &[u32], out_dir: impl AsRef<Path>) -> Result<()> {
        let out_str = out_dir.as_ref().to_string_lossy().to_string();
        let result = self.inner.extract_items_to_dir(indices, &out_str);
        self.check_result(result)
    }

    /// Extract all items to a directory.
    pub fn extract_all(&self, out_dir: impl AsRef<Path>) -> Result<()> {
        let out_str = out_dir.as_ref().to_string_lossy().to_string();
        let result = self.inner.extract_all_to_dir(&out_str);
        self.check_result(result)
    }

    /// Extract items matching a wildcard pattern to a directory.
    pub fn extract_matching(&self, pattern: &str, out_dir: impl AsRef<Path>) -> Result<()> {
        let out_str = out_dir.as_ref().to_string_lossy().to_string();
        let result = self.inner.extract_matching(pattern, &out_str);
        self.check_result(result)
    }

    /// Test archive integrity.
    pub fn test(&self) -> Result<()> {
        let result = self.inner.test();
        self.check_result(result)
    }

    /// Set password for encrypted archives.
    pub fn set_password(&mut self, password: &str) {
        self.inner.pin_mut().set_password(password);
    }

    fn check_result(&self, result: ffi::OperationResult) -> Result<()> {
        match result.code {
            ffi::ErrorCode::Ok => Ok(()),
            ffi::ErrorCode::InvalidPassword => Err(Error::InvalidPassword),
            ffi::ErrorCode::ExtractionFailed => Err(Error::Extraction(result.message)),
            ffi::ErrorCode::ArchiveOpenFailed => Err(Error::ArchiveOpen(result.message)),
            ffi::ErrorCode::IoError => Err(Error::Io(result.message)),
            _ => Err(Error::Unknown(result.message)),
        }
    }
}

/// Check if an archive file is encrypted.
pub fn is_archive_encrypted(lib: &Library, path: impl AsRef<Path>, format: ArchiveFormat) -> bool {
    let path_str = path.as_ref().to_string_lossy().to_string();
    ffi::is_archive_encrypted(&lib.inner, &path_str, format)
}

/// Check if an archive header is encrypted (requires password to list contents).
pub fn is_header_encrypted(lib: &Library, path: impl AsRef<Path>, format: ArchiveFormat) -> bool {
    let path_str = path.as_ref().to_string_lossy().to_string();
    ffi::is_header_encrypted(&lib.inner, &path_str, format)
}

/// Detect archive format from file extension.
pub fn detect_format(lib: &Library, path: impl AsRef<Path>) -> ArchiveFormat {
    let path_str = path.as_ref().to_string_lossy().to_string();
    ffi::detect_format(&lib.inner, &path_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_from_code() {
        let err: Error = ffi::ErrorCode::InvalidPassword.into();
        assert!(matches!(err, Error::InvalidPassword));
    }

    #[test]
    fn test_load_library() {
        let dll_path = std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .join("output")
            .join("7z.dll");

        if dll_path.exists() {
            let lib = Library::new(&dll_path);
            assert!(lib.is_ok(), "Failed to load library: {:?}", lib.err());
        }
    }

    #[test]
    fn test_read_archive() {
        let workspace_root = std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let dll_path = workspace_root.join("output").join("7z.dll");
        let archive_path = workspace_root.join("test_data").join("test.7z");

        if !dll_path.exists() || !archive_path.exists() {
            return;
        }

        let lib = Library::new(&dll_path).expect("Failed to load library");
        let reader = ArchiveReader::open(&lib, &archive_path, ArchiveFormat::SevenZip, "")
            .expect("Failed to open archive");

        let metadata = reader.metadata();
        assert_eq!(metadata.files_count, 1);

        let items = reader.items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "hello.txt");
        assert_eq!(items[0].size, 14);
    }

    #[test]
    fn test_extract_to_buffer() {
        let workspace_root = std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let dll_path = workspace_root.join("output").join("7z.dll");
        let archive_path = workspace_root.join("test_data").join("test.7z");

        if !dll_path.exists() || !archive_path.exists() {
            return;
        }

        let lib = Library::new(&dll_path).expect("Failed to load library");
        let reader = ArchiveReader::open(&lib, &archive_path, ArchiveFormat::SevenZip, "")
            .expect("Failed to open archive");

        let data = reader.extract_to_buffer(0).expect("Failed to extract");
        let content = String::from_utf8_lossy(&data);
        assert!(content.contains("Hello"));
    }
}
