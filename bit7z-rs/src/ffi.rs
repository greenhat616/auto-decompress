//! FFI bindings to the bit7z C++ wrapper

#[cxx::bridge(namespace = "bit7z_wrapper")]
mod bridge {
    /// Archive format enum
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum ArchiveFormat {
        Auto = 0,
        Zip,
        SevenZip,
        Rar,
        Rar5,
        GZip,
        BZip2,
        Xz,
        Lzma,
        Tar,
        Wim,
        Iso,
        Cab,
        Arj,
        Z,
        Lzh,
        Nsis,
        Cpio,
        Rpm,
        Deb,
        Dmg,
        Hfs,
        Xar,
        Vhd,
        Fat,
        Ntfs,
        Split,
    }

    /// Error codes for operations
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ErrorCode {
        Ok = 0,
        LibraryLoadFailed,
        ArchiveOpenFailed,
        InvalidPassword,
        ExtractionFailed,
        InvalidIndex,
        InvalidFormat,
        IoError,
        UnknownError,
    }

    /// Information about a single archive item
    #[derive(Debug, Clone)]
    pub struct ArchiveItemInfo {
        pub path: String,
        pub name: String,
        pub size: u64,
        pub packed_size: u64,
        pub crc: u64,
        pub creation_time: i64,
        pub modification_time: i64,
        pub access_time: i64,
        pub attributes: u32,
        pub index: u32,
        pub is_dir: bool,
        pub is_encrypted: bool,
    }

    /// Archive metadata
    #[derive(Debug, Clone)]
    pub struct ArchiveInfo {
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

    /// Result of an operation
    #[derive(Debug, Clone)]
    pub struct OperationResult {
        pub code: ErrorCode,
        pub message: String,
    }

    unsafe extern "C++" {
        include!("bit7z-rs/cpp/bit7z_bridge.hpp");

        /// The 7z library handle
        type Bit7zLibrary;

        /// Archive reader handle
        type ArchiveReader;

        // Library functions
        fn new_library(dll_path: &str) -> UniquePtr<Bit7zLibrary>;
        fn is_valid(self: &Bit7zLibrary) -> bool;
        fn get_error(self: &Bit7zLibrary) -> String;

        // Archive reader construction
        fn new_archive_reader(
            lib: &UniquePtr<Bit7zLibrary>,
            archive_path: &str,
            format: ArchiveFormat,
            password: &str,
        ) -> UniquePtr<ArchiveReader>;

        // Archive reader methods
        fn is_valid(self: &ArchiveReader) -> bool;
        fn get_error(self: &ArchiveReader) -> String;
        fn get_archive_info(self: &ArchiveReader) -> ArchiveInfo;
        fn get_items(self: &ArchiveReader) -> Vec<ArchiveItemInfo>;
        fn get_item(self: &ArchiveReader, index: u32) -> ArchiveItemInfo;
        fn is_encrypted(self: &ArchiveReader) -> bool;
        fn is_header_encrypted(self: &ArchiveReader) -> bool;
        fn extract_to_dir(self: &ArchiveReader, index: u32, out_dir: &str) -> OperationResult;
        fn extract_to_buffer(
            self: &ArchiveReader,
            index: u32,
            out_buffer: &mut Vec<u8>,
        ) -> OperationResult;
        fn extract_items_to_dir(
            self: &ArchiveReader,
            indices: &[u32],
            out_dir: &str,
        ) -> OperationResult;
        fn extract_all_to_dir(self: &ArchiveReader, out_dir: &str) -> OperationResult;
        fn extract_matching(self: &ArchiveReader, pattern: &str, out_dir: &str) -> OperationResult;
        fn test(self: &ArchiveReader) -> OperationResult;
        fn set_password(self: Pin<&mut ArchiveReader>, password: &str);

        // Static utility functions
        fn is_archive_encrypted(
            lib: &UniquePtr<Bit7zLibrary>,
            archive_path: &str,
            format: ArchiveFormat,
        ) -> bool;
        fn is_header_encrypted(
            lib: &UniquePtr<Bit7zLibrary>,
            archive_path: &str,
            format: ArchiveFormat,
        ) -> bool;
        fn detect_format(lib: &UniquePtr<Bit7zLibrary>, archive_path: &str) -> ArchiveFormat;
    }
}

pub use bridge::*;
