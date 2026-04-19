#pragma once

#include <cstdint>
#include <memory>
#include <string>
#include <vector>

#include "rust/cxx.h"

namespace bit7z_wrapper {

// Types from the FFI - these must match ffi.rs exactly
struct ArchiveItemInfo;
struct ArchiveInfo;
struct OperationResult;
enum class ArchiveFormat : uint8_t;
enum class ErrorCode : uint8_t;

// Forward declaration of implementation classes
class Bit7zLibraryImpl;
class ArchiveReaderImpl;

// Library wrapper with full definition for cxx opaque type
class Bit7zLibrary {
public:
    Bit7zLibrary();
    ~Bit7zLibrary();

    bool is_valid() const;
    rust::String get_error() const;

    // Internal access for other functions
    Bit7zLibraryImpl* impl() const { return impl_.get(); }

    // Allow factory function access to internals
    void set_valid(bool v) { valid_ = v; }
    void set_error(const std::string& e) { error_ = e; }

private:
    std::unique_ptr<Bit7zLibraryImpl> impl_;
    std::string error_;
    bool valid_ = false;
};

// Archive reader with full definition for cxx opaque type
class ArchiveReader {
public:
    ArchiveReader();
    ~ArchiveReader();

    bool is_valid() const;
    rust::String get_error() const;
    ArchiveInfo get_archive_info() const;
    rust::Vec<ArchiveItemInfo> get_items() const;
    ArchiveItemInfo get_item(uint32_t index) const;
    bool is_encrypted() const;
    bool is_header_encrypted() const;
    OperationResult extract_to_dir(uint32_t index, rust::Str out_dir) const;
    OperationResult extract_to_buffer(uint32_t index, rust::Vec<uint8_t>& out_buffer) const;
    OperationResult extract_items_to_dir(rust::Slice<const uint32_t> indices, rust::Str out_dir) const;
    OperationResult extract_all_to_dir(rust::Str out_dir) const;
    OperationResult extract_matching(rust::Str pattern, rust::Str out_dir) const;
    OperationResult test() const;
    void set_password(rust::Str password);

    // Internal access
    ArchiveReaderImpl* impl() const { return impl_.get(); }

    // Allow factory function access to internals
    void set_valid(bool v) { valid_ = v; }
    void set_error(const std::string& e) { error_ = e; }

private:
    std::unique_ptr<ArchiveReaderImpl> impl_;
    std::string error_;
    bool valid_ = false;
};

// Factory functions
std::unique_ptr<Bit7zLibrary> new_library(rust::Str dll_path);

std::unique_ptr<ArchiveReader> new_archive_reader(
    const std::unique_ptr<Bit7zLibrary>& lib,
    rust::Str archive_path,
    ArchiveFormat format,
    rust::Str password
);

// Static utility functions
bool is_archive_encrypted(
    const std::unique_ptr<Bit7zLibrary>& lib,
    rust::Str archive_path,
    ArchiveFormat format
);

bool is_header_encrypted(
    const std::unique_ptr<Bit7zLibrary>& lib,
    rust::Str archive_path,
    ArchiveFormat format
);

ArchiveFormat detect_format(
    const std::unique_ptr<Bit7zLibrary>& lib,
    rust::Str archive_path
);

} // namespace bit7z_wrapper
