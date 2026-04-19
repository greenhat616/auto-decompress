#include "bit7z-rs/cpp/bit7z_bridge.hpp"
#include "bit7z-rs/src/ffi.rs.h"

#include <bit7z/bit7z.hpp>
#include <bit7z/bitarchivereader.hpp>
#include <bit7z/bitfileextractor.hpp>
#include <bit7z/bitexception.hpp>

#include <filesystem>
#include <algorithm>
#include <cctype>

namespace bit7z_wrapper {

namespace {

inline std::string to_std_string(rust::Str s) {
    return std::string(s.data(), s.size());
}

const bit7z::BitInFormat& to_bit7z_format(ArchiveFormat format) {
    switch (format) {
        case ArchiveFormat::Zip: return bit7z::BitFormat::Zip;
        case ArchiveFormat::SevenZip: return bit7z::BitFormat::SevenZip;
        case ArchiveFormat::Rar: return bit7z::BitFormat::Rar;
        case ArchiveFormat::Rar5: return bit7z::BitFormat::Rar5;
        case ArchiveFormat::GZip: return bit7z::BitFormat::GZip;
        case ArchiveFormat::BZip2: return bit7z::BitFormat::BZip2;
        case ArchiveFormat::Xz: return bit7z::BitFormat::Xz;
        case ArchiveFormat::Lzma: return bit7z::BitFormat::Lzma;
        case ArchiveFormat::Tar: return bit7z::BitFormat::Tar;
        case ArchiveFormat::Wim: return bit7z::BitFormat::Wim;
        case ArchiveFormat::Iso: return bit7z::BitFormat::Iso;
        case ArchiveFormat::Cab: return bit7z::BitFormat::Cab;
        case ArchiveFormat::Arj: return bit7z::BitFormat::Arj;
        case ArchiveFormat::Z: return bit7z::BitFormat::Z;
        case ArchiveFormat::Lzh: return bit7z::BitFormat::Lzh;
        case ArchiveFormat::Nsis: return bit7z::BitFormat::Nsis;
        case ArchiveFormat::Cpio: return bit7z::BitFormat::Cpio;
        case ArchiveFormat::Rpm: return bit7z::BitFormat::Rpm;
        case ArchiveFormat::Deb: return bit7z::BitFormat::Deb;
        case ArchiveFormat::Dmg: return bit7z::BitFormat::Dmg;
        case ArchiveFormat::Hfs: return bit7z::BitFormat::Hfs;
        case ArchiveFormat::Xar: return bit7z::BitFormat::Xar;
        case ArchiveFormat::Vhd: return bit7z::BitFormat::Vhd;
        case ArchiveFormat::Fat: return bit7z::BitFormat::Fat;
        case ArchiveFormat::Ntfs: return bit7z::BitFormat::Ntfs;
        case ArchiveFormat::Split: return bit7z::BitFormat::Split;
        case ArchiveFormat::Auto:
        default:
#ifdef BIT7Z_AUTO_FORMAT
            return bit7z::BitFormat::Auto;
#else
            return bit7z::BitFormat::SevenZip;
#endif
    }
}

int64_t filetime_to_unix(const bit7z::BitPropVariant& prop) {
    if (prop.isEmpty() || !prop.isFileTime()) {
        return -1;
    }
    try {
        auto ft = prop.getFileTime();
        constexpr int64_t FILETIME_UNIX_DIFF = 116444736000000000LL;
        int64_t filetime_value = (static_cast<int64_t>(ft.dwHighDateTime) << 32) | ft.dwLowDateTime;
        return (filetime_value - FILETIME_UNIX_DIFF) / 10000000LL;
    } catch (...) {
        return -1;
    }
}

} // anonymous namespace

// PIMPL implementation classes
class Bit7zLibraryImpl {
public:
    std::unique_ptr<bit7z::Bit7zLibrary> lib;
};

class ArchiveReaderImpl {
public:
    std::unique_ptr<bit7z::BitArchiveReader> reader;
    std::unique_ptr<bit7z::BitFileExtractor> extractor;
    bit7z::tstring archive_path_str;
    bit7z::tstring password_str;
    const bit7z::BitInFormat* format_ptr = nullptr;
};

// Bit7zLibrary implementation
Bit7zLibrary::Bit7zLibrary() : impl_(std::make_unique<Bit7zLibraryImpl>()) {}
Bit7zLibrary::~Bit7zLibrary() = default;

bool Bit7zLibrary::is_valid() const { return valid_; }
rust::String Bit7zLibrary::get_error() const { return rust::String(error_); }

// ArchiveReader implementation
ArchiveReader::ArchiveReader() : impl_(std::make_unique<ArchiveReaderImpl>()) {}
ArchiveReader::~ArchiveReader() = default;

bool ArchiveReader::is_valid() const { return valid_; }
rust::String ArchiveReader::get_error() const { return rust::String(error_); }

ArchiveInfo ArchiveReader::get_archive_info() const {
    ArchiveInfo info{};
    if (!valid_ || !impl_->reader) return info;

    try {
        info.size = impl_->reader->size();
        info.packed_size = impl_->reader->packSize();
        info.items_count = impl_->reader->filesCount() + impl_->reader->foldersCount();
        info.folders_count = impl_->reader->foldersCount();
        info.files_count = impl_->reader->filesCount();
        info.volumes_count = impl_->reader->volumesCount();
        info.is_encrypted = impl_->reader->isEncrypted();
        info.has_encrypted_items = impl_->reader->hasEncryptedItems();
        info.is_multi_volume = impl_->reader->isMultiVolume();
        info.is_solid = impl_->reader->isSolid();
    } catch (...) {}

    return info;
}

rust::Vec<ArchiveItemInfo> ArchiveReader::get_items() const {
    rust::Vec<ArchiveItemInfo> items;
    if (!valid_ || !impl_->reader) return items;

    try {
        auto bit7z_items = impl_->reader->items();
        uint32_t idx = 0;

        for (const auto& item : bit7z_items) {
            ArchiveItemInfo info{};

            info.path = rust::String(item.path());
            info.name = rust::String(item.name());
            info.size = item.size();
            info.is_dir = item.isDir();
            info.attributes = item.attributes();

            auto props = item.itemProperties();

            auto packed_it = props.find(bit7z::BitProperty::PackSize);
            if (packed_it != props.end() && packed_it->second.isUInt64()) {
                info.packed_size = packed_it->second.getUInt64();
            }

            auto crc_it = props.find(bit7z::BitProperty::CRC);
            if (crc_it != props.end() && crc_it->second.isUInt32()) {
                info.crc = crc_it->second.getUInt32();
            }

            auto ctime_it = props.find(bit7z::BitProperty::CTime);
            info.creation_time = filetime_to_unix(
                ctime_it != props.end() ? ctime_it->second : bit7z::BitPropVariant{}
            );

            auto mtime_it = props.find(bit7z::BitProperty::MTime);
            info.modification_time = filetime_to_unix(
                mtime_it != props.end() ? mtime_it->second : bit7z::BitPropVariant{}
            );

            auto atime_it = props.find(bit7z::BitProperty::ATime);
            info.access_time = filetime_to_unix(
                atime_it != props.end() ? atime_it->second : bit7z::BitPropVariant{}
            );

            auto enc_it = props.find(bit7z::BitProperty::Encrypted);
            if (enc_it != props.end() && enc_it->second.isBool()) {
                info.is_encrypted = enc_it->second.getBool();
            }

            info.index = idx++;
            items.push_back(std::move(info));
        }
    } catch (...) {}

    return items;
}

ArchiveItemInfo ArchiveReader::get_item(uint32_t index) const {
    auto all_items = get_items();
    if (index < all_items.size()) {
        return std::move(all_items[index]);
    }
    return ArchiveItemInfo{};
}

bool ArchiveReader::is_encrypted() const {
    if (!valid_ || !impl_->reader) return false;
    try {
        return impl_->reader->isEncrypted();
    } catch (...) {
        return false;
    }
}

bool ArchiveReader::is_header_encrypted() const {
    return false;
}

OperationResult ArchiveReader::extract_to_dir(uint32_t index, rust::Str out_dir) const {
    OperationResult result{ErrorCode::Ok, rust::String()};
    if (!valid_ || !impl_->extractor) {
        result.code = ErrorCode::ArchiveOpenFailed;
        result.message = rust::String("Archive not opened");
        return result;
    }

    try {
        bit7z::tstring out(to_std_string(out_dir));
        std::vector<uint32_t> indices{index};
        impl_->extractor->extractItems(impl_->archive_path_str, indices, out);
    } catch (const bit7z::BitException& e) {
        result.code = ErrorCode::ExtractionFailed;
        result.message = rust::String(e.what());
    } catch (const std::exception& e) {
        result.code = ErrorCode::UnknownError;
        result.message = rust::String(e.what());
    }

    return result;
}

OperationResult ArchiveReader::extract_to_buffer(uint32_t index, rust::Vec<uint8_t>& out_buffer) const {
    OperationResult result{ErrorCode::Ok, rust::String()};
    if (!valid_ || !impl_->extractor) {
        result.code = ErrorCode::ArchiveOpenFailed;
        result.message = rust::String("Archive not opened");
        return result;
    }

    try {
        std::vector<bit7z::byte_t> buffer;
        impl_->extractor->extract(impl_->archive_path_str, buffer, index);
        out_buffer.clear();
        out_buffer.reserve(buffer.size());
        for (auto b : buffer) {
            out_buffer.push_back(b);
        }
    } catch (const bit7z::BitException& e) {
        result.code = ErrorCode::ExtractionFailed;
        result.message = rust::String(e.what());
    } catch (const std::exception& e) {
        result.code = ErrorCode::UnknownError;
        result.message = rust::String(e.what());
    }

    return result;
}

OperationResult ArchiveReader::extract_items_to_dir(rust::Slice<const uint32_t> indices, rust::Str out_dir) const {
    OperationResult result{ErrorCode::Ok, rust::String()};
    if (!valid_ || !impl_->extractor) {
        result.code = ErrorCode::ArchiveOpenFailed;
        result.message = rust::String("Archive not opened");
        return result;
    }

    try {
        bit7z::tstring out(to_std_string(out_dir));
        std::vector<uint32_t> idx_vec(indices.begin(), indices.end());
        impl_->extractor->extractItems(impl_->archive_path_str, idx_vec, out);
    } catch (const bit7z::BitException& e) {
        result.code = ErrorCode::ExtractionFailed;
        result.message = rust::String(e.what());
    } catch (const std::exception& e) {
        result.code = ErrorCode::UnknownError;
        result.message = rust::String(e.what());
    }

    return result;
}

OperationResult ArchiveReader::extract_all_to_dir(rust::Str out_dir) const {
    OperationResult result{ErrorCode::Ok, rust::String()};
    if (!valid_ || !impl_->extractor) {
        result.code = ErrorCode::ArchiveOpenFailed;
        result.message = rust::String("Archive not opened");
        return result;
    }

    try {
        bit7z::tstring out(to_std_string(out_dir));
        impl_->extractor->extract(impl_->archive_path_str, out);
    } catch (const bit7z::BitException& e) {
        result.code = ErrorCode::ExtractionFailed;
        result.message = rust::String(e.what());
    } catch (const std::exception& e) {
        result.code = ErrorCode::UnknownError;
        result.message = rust::String(e.what());
    }

    return result;
}

OperationResult ArchiveReader::extract_matching(rust::Str pattern, rust::Str out_dir) const {
    OperationResult result{ErrorCode::Ok, rust::String()};
    if (!valid_ || !impl_->extractor) {
        result.code = ErrorCode::ArchiveOpenFailed;
        result.message = rust::String("Archive not opened");
        return result;
    }

    try {
        bit7z::tstring pat(to_std_string(pattern));
        bit7z::tstring out(to_std_string(out_dir));
        impl_->extractor->extractMatching(impl_->archive_path_str, pat, out);
    } catch (const bit7z::BitException& e) {
        result.code = ErrorCode::ExtractionFailed;
        result.message = rust::String(e.what());
    } catch (const std::exception& e) {
        result.code = ErrorCode::UnknownError;
        result.message = rust::String(e.what());
    }

    return result;
}

OperationResult ArchiveReader::test() const {
    OperationResult result{ErrorCode::Ok, rust::String()};
    if (!valid_ || !impl_->extractor) {
        result.code = ErrorCode::ArchiveOpenFailed;
        result.message = rust::String("Archive not opened");
        return result;
    }

    try {
        impl_->extractor->test(impl_->archive_path_str);
    } catch (const bit7z::BitException& e) {
        result.code = ErrorCode::ExtractionFailed;
        result.message = rust::String(e.what());
    } catch (const std::exception& e) {
        result.code = ErrorCode::UnknownError;
        result.message = rust::String(e.what());
    }

    return result;
}

void ArchiveReader::set_password(rust::Str password) {
    impl_->password_str = to_std_string(password);
    if (impl_->extractor) {
        impl_->extractor->setPassword(impl_->password_str);
    }
}

// Factory functions
std::unique_ptr<Bit7zLibrary> new_library(rust::Str dll_path) {
    auto wrapper = std::make_unique<Bit7zLibrary>();
    bit7z::tstring path(to_std_string(dll_path));

    try {
        wrapper->impl()->lib = std::make_unique<bit7z::Bit7zLibrary>(path);
        wrapper->set_valid(true);
    } catch (const bit7z::BitException& e) {
        wrapper->set_error(e.what());
    } catch (const std::exception& e) {
        wrapper->set_error(e.what());
    }

    return wrapper;
}

std::unique_ptr<ArchiveReader> new_archive_reader(
    const std::unique_ptr<Bit7zLibrary>& lib,
    rust::Str archive_path,
    ArchiveFormat format,
    rust::Str password
) {
    auto wrapper = std::make_unique<ArchiveReader>();

    if (!lib || !lib->is_valid()) {
        wrapper->set_error("Invalid library");
        return wrapper;
    }

    wrapper->impl()->archive_path_str = to_std_string(archive_path);
    wrapper->impl()->password_str = to_std_string(password);
    wrapper->impl()->format_ptr = &to_bit7z_format(format);

    try {
        wrapper->impl()->reader = std::make_unique<bit7z::BitArchiveReader>(
            *lib->impl()->lib, wrapper->impl()->archive_path_str, *wrapper->impl()->format_ptr, wrapper->impl()->password_str
        );
        wrapper->impl()->extractor = std::make_unique<bit7z::BitFileExtractor>(*lib->impl()->lib, *wrapper->impl()->format_ptr);
        if (!wrapper->impl()->password_str.empty()) {
            wrapper->impl()->extractor->setPassword(wrapper->impl()->password_str);
        }
        wrapper->set_valid(true);
    } catch (const bit7z::BitException& e) {
        wrapper->set_error(e.what());
    } catch (const std::exception& e) {
        wrapper->set_error(e.what());
    }

    return wrapper;
}

// Static utility functions
bool is_archive_encrypted(
    const std::unique_ptr<Bit7zLibrary>& lib,
    rust::Str archive_path,
    ArchiveFormat format
) {
    if (!lib || !lib->is_valid()) return false;

    try {
        bit7z::tstring path(to_std_string(archive_path));
        return bit7z::BitArchiveReader::isEncrypted(*lib->impl()->lib, path, to_bit7z_format(format));
    } catch (...) {
        return false;
    }
}

bool is_header_encrypted(
    const std::unique_ptr<Bit7zLibrary>& lib,
    rust::Str archive_path,
    ArchiveFormat format
) {
    if (!lib || !lib->is_valid()) return false;

    try {
        bit7z::tstring path(to_std_string(archive_path));
        return bit7z::BitArchiveReader::isHeaderEncrypted(*lib->impl()->lib, path, to_bit7z_format(format));
    } catch (...) {
        return false;
    }
}

ArchiveFormat detect_format(
    const std::unique_ptr<Bit7zLibrary>& lib,
    rust::Str archive_path
) {
    std::string path = to_std_string(archive_path);
    std::filesystem::path p(path);
    auto ext = p.extension().string();

    std::transform(ext.begin(), ext.end(), ext.begin(),
        [](unsigned char c) { return static_cast<char>(std::tolower(c)); });

    if (ext == ".7z") return ArchiveFormat::SevenZip;
    if (ext == ".zip") return ArchiveFormat::Zip;
    if (ext == ".rar") return ArchiveFormat::Rar;
    if (ext == ".gz" || ext == ".gzip") return ArchiveFormat::GZip;
    if (ext == ".bz2" || ext == ".bzip2") return ArchiveFormat::BZip2;
    if (ext == ".xz") return ArchiveFormat::Xz;
    if (ext == ".lzma") return ArchiveFormat::Lzma;
    if (ext == ".tar") return ArchiveFormat::Tar;
    if (ext == ".wim") return ArchiveFormat::Wim;
    if (ext == ".iso") return ArchiveFormat::Iso;
    if (ext == ".cab") return ArchiveFormat::Cab;
    if (ext == ".rpm") return ArchiveFormat::Rpm;
    if (ext == ".deb") return ArchiveFormat::Deb;
    if (ext == ".dmg") return ArchiveFormat::Dmg;

    return ArchiveFormat::Auto;
}

} // namespace bit7z_wrapper
