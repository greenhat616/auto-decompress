# bit7z-rs

Rust bindings for [bit7z](https://github.com/rikyoz/bit7z) - a C++ static library offering a clean
and simple interface to the 7-zip shared libraries.

## Features

- Read encrypted and multi-volume archives
- List archive contents with full metadata
- Extract specific files by index or wildcard pattern
- Support for many archive formats: 7z, zip, rar, tar, gzip, bzip2, xz, iso, cab, and more

## Prerequisites

### 1. Build bit7z

Clone and build bit7z from source:

```bash
git clone https://github.com/rikyoz/bit7z.git
cd bit7z
cmake -B build -DCMAKE_BUILD_TYPE=Release -DBIT7Z_AUTO_FORMAT=ON
cmake --build build --config Release
cmake --install build --prefix /path/to/install
```

### 2. Get 7-zip DLLs

Download the 7-zip SDK or extract DLLs from 7-zip installation:

- Windows: `7z.dll` (from 7-zip installation)
- Linux: `7z.so` (from p7zip package)

### 3. Set Environment Variables

```bash
# Point to bit7z installation
export BIT7Z_DIR=/path/to/bit7z/install

# Optional: enable auto format detection
export BIT7Z_AUTO_FORMAT=1
```

## Usage

```rust
use bit7z_rs::{Library, ArchiveReader, ArchiveFormat};

fn main() -> bit7z_rs::Result<()> {
    // Load the 7z library
    let lib = Library::new("7z.dll")?;

    // Open an archive
    let reader = ArchiveReader::open(
        &lib,
        "archive.7z",
        ArchiveFormat::Auto,
        "", // password (empty if none)
    )?;

    // Get archive metadata
    let metadata = reader.metadata();
    println!("Files: {}, Size: {}", metadata.files_count, metadata.size);

    // List all items
    for item in reader.items() {
        println!("{}: {} bytes", item.path, item.size);
    }

    // Extract a specific file by index
    reader.extract_to_dir(0, "output/")?;

    // Extract all files
    reader.extract_all("output/")?;

    // Extract to memory
    let data = reader.extract_to_buffer(0)?;
    println!("Extracted {} bytes", data.len());

    Ok(())
}
```

## Encrypted Archives

```rust
let reader = ArchiveReader::open(
    &lib,
    "encrypted.7z",
    ArchiveFormat::SevenZip,
    "my_password",
)?;
```

## Multi-volume Archives

```rust
// Just open the first volume - bit7z handles the rest
let reader = ArchiveReader::open_multi_volume(
    &lib,
    "archive.7z.001",
    ArchiveFormat::SevenZip,
    "",
)?;
```

## Supported Formats

| Format           | Extension | Read | Notes                             |
| ---------------- | --------- | ---- | --------------------------------- |
| 7z               | .7z       | Yes  | Full support including encryption |
| ZIP              | .zip      | Yes  | Full support including encryption |
| RAR              | .rar      | Yes  | Read-only                         |
| RAR5             | .rar      | Yes  | Read-only                         |
| TAR              | .tar      | Yes  |                                   |
| GZip             | .gz       | Yes  |                                   |
| BZip2            | .bz2      | Yes  |                                   |
| XZ               | .xz       | Yes  |                                   |
| LZMA             | .lzma     | Yes  |                                   |
| ISO              | .iso      | Yes  |                                   |
| WIM              | .wim      | Yes  |                                   |
| CAB              | .cab      | Yes  |                                   |
| And many more... |           |      |                                   |

## License

MIT OR Apache-2.0
