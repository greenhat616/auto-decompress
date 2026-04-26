use camino::Utf8Path;
use file_type::FileType;
use fs_err as fs;
use std::borrow::Cow;

use super::{Error, FileTypeDetector, InputKind};

pub struct NormalFileTypeDetector;

impl FileTypeDetector for NormalFileTypeDetector {
    fn accepted_input_kinds(&self) -> Vec<InputKind> {
        vec![InputKind::Bytes, InputKind::Path]
    }

    fn detect(&self, input: &[u8], extension: Option<&str>) -> Result<FileType, Error> {
        Ok(detect_file_type(input, extension))
    }

    fn detect_from_path(&self, path: &Utf8Path) -> Result<FileType, Error> {
        detect_type_from_path(path)
    }
}

#[allow(dead_code)]
pub fn normalize_extension(filename: &str) -> Cow<'_, str> {
    let Some((name, extension)) = filename.split_once('.') else {
        return Cow::Borrowed(filename);
    };

    let extension = extension
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.')
        .map(|c| c.to_ascii_lowercase())
        .collect::<String>();

    Cow::Owned(format!("{name}.{extension}"))
}

pub fn detect_file_type(bytes: &[u8], extension: Option<&str>) -> FileType {
    let file_type = FileType::from_bytes(bytes).to_owned();
    if let Some(extension) = extension
        && !file_type.extensions().contains(&extension)
    {
        tracing::warn!(
            "detected file type {} does not match extension {}",
            file_type.name(),
            extension
        );
    }
    file_type
}

pub fn detect_type_from_path(path: &Utf8Path) -> Result<FileType, Error> {
    let extension = path.extension();
    let bytes = fs::read(path)?;
    Ok(detect_file_type(&bytes, extension))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test_log::test]
    fn test_accepted_input_kinds() {
        let detector = NormalFileTypeDetector;

        assert_eq!(
            detector.accepted_input_kinds(),
            vec![InputKind::Bytes, InputKind::Path]
        );
    }

    #[test_log::test]
    fn test_normalize_extension() {
        assert_eq!(normalize_extension("test.exe"), "test.exe");
        assert_eq!(normalize_extension(".env"), ".env");
        assert_eq!(normalize_extension("test.删去txt.删zip除"), "test.txt.zip");
    }

    #[test_log::test]
    fn test_detect_file_type() {
        let file_type = detect_file_type(b"A Very Very Very Long text", None);
        assert_eq!(file_type.name(), "Text");
    }

    #[test_log::test]
    fn test_detect_file_type_prefers_magic_over_spoofed_extension() {
        let file_type = detect_file_type(b"A Very Very Very Long text", Some("jpg"));
        assert_eq!(file_type.name(), "Text");
    }

    #[test_log::test]
    fn test_detect_type_from_path() {
        use tempfile::TempDir;
        use zip::ZipWriter;

        // 创建临时目录
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // 创建一个空 zip 文件，并添加 .jpg 后缀
        let zip_path = temp_path.join("test.jpg");
        let file = fs::File::create(&zip_path).unwrap();
        let zip = ZipWriter::new(file);
        zip.finish().unwrap();

        // 使用 detect_type_from_path 识别文件类型
        let path = Utf8Path::from_path(&zip_path).unwrap();
        let file_type = detect_type_from_path(path).unwrap();

        // 验证识别结果为 zip
        assert_eq!(file_type.name(), "ZIP Format");
    }

    #[test_log::test]
    fn test_detect_type_from_path_returns_io_error_for_missing_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let missing = temp_dir.path().join("missing.zip");
        let path = Utf8Path::from_path(&missing).unwrap();

        assert!(matches!(detect_type_from_path(path), Err(Error::Io(_))));
    }

    #[test_log::test]
    fn test_detect_split_zip() {
        use tempfile::TempDir;
        use zip::ZipWriter;

        // 创建临时目录
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // 创建分卷 zip 文件（使用最小化空 zip，确保 file_type 能稳定识别）
        // 第一部分：.zip 文件（空内容）
        let zip_path = temp_path.join("test.zip");
        {
            let file = fs::File::create(&zip_path).unwrap();
            let zip = ZipWriter::new(file);
            zip.finish().unwrap();
        } // 确保文件被关闭

        // 使用 detect_type_from_path 识别主 zip 文件
        let path = Utf8Path::from_path(&zip_path).unwrap();
        let file_type = detect_type_from_path(path).unwrap();
        assert_eq!(file_type.name(), "ZIP Format");

        // 创建分卷文件 .z01（使用实际 zip 数据模拟分卷内容）
        // 将完整 zip 内容复制到 .z01，确保文件头一致，便于类型识别
        let z01_path = temp_path.join("test.z01");
        let zip_bytes = fs::read(&zip_path).unwrap();
        fs::write(&z01_path, zip_bytes).unwrap();

        // 识别 .z01 文件类型
        let z01_path_utf8 = Utf8Path::from_path(&z01_path).unwrap();
        let z01_file_type = detect_type_from_path(z01_path_utf8).unwrap();
        // file_type 应该能够识别包含 ZIP 签名的文件
        assert_eq!(z01_file_type.name(), "ZIP Format");
    }
}
