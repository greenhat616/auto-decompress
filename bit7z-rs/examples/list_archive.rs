//! Example: List contents of an archive

use bit7z_rs::{ArchiveFormat, ArchiveReader, Library};
use std::env;

fn main() -> bit7z_rs::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <archive_path> [password]", args[0]);
        std::process::exit(1);
    }

    let archive_path = &args[1];
    let password = args.get(2).map(|s| s.as_str()).unwrap_or("");

    // Load the 7z library
    let lib = Library::load_default()?;

    // Open the archive
    let reader = ArchiveReader::open(&lib, archive_path, ArchiveFormat::Auto, password)?;

    // Print archive metadata
    let meta = reader.metadata();
    println!("Archive: {}", archive_path);
    println!(
        "Files: {}, Folders: {}",
        meta.files_count, meta.folders_count
    );
    println!(
        "Size: {} bytes, Packed: {} bytes",
        meta.size, meta.packed_size
    );
    println!("Encrypted: {}, Solid: {}", meta.is_encrypted, meta.is_solid);
    if meta.is_multi_volume {
        println!("Multi-volume: {} volumes", meta.volumes_count);
    }
    println!();

    // List all items
    println!(
        "{:<6} {:<12} {:<12} {:<5} Path",
        "Index", "Size", "Packed", "Dir"
    );
    println!("{}", "-".repeat(60));

    for item in reader.items() {
        println!(
            "{:<6} {:<12} {:<12} {:<5} {}",
            item.index,
            item.size,
            item.packed_size,
            if item.is_dir { "yes" } else { "no" },
            item.path
        );
    }

    Ok(())
}
