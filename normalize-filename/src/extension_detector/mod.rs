use camino::Utf8Path;
use file_type::FileType;

mod magika;
mod normal;

pub use normal::NormalFileTypeDetector;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    Bytes,
    Path,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("file type detection failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported input kind: {0:?}")]
    UnsupportedInputKind(InputKind),
}

pub trait FileTypeDetector {
    /// Returns the kinds of input this detector can accept (e.g., bytes, file paths).
    fn accepted_input_kinds(&self) -> Vec<InputKind>;
    fn detect(&self, input: &[u8], extension: Option<&str>) -> Result<FileType, Error>;
    fn detect_from_path(&self, path: &Utf8Path) -> Result<FileType, Error>;
}
