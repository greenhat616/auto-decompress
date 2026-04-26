use std::{borrow::Cow, collections::HashSet};

pub mod extension_detector;

pub fn normalize_extension(filename: &str) -> Cow<'_, str> {
    let mut parts = filename.split('.');
    let Some(stem) = parts.next() else {
        return Cow::Borrowed(filename);
    };
    let extension_parts = parts.collect::<Vec<_>>();
    if extension_parts.is_empty() {
        return Cow::Borrowed(filename);
    }

    let mut seen = HashSet::new();
    let mut normalized_parts = Vec::new();
    let mut changed = false;

    for part in extension_parts.iter().rev() {
        let normalized = part
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .collect::<String>();

        if normalized != *part {
            changed = true;
        }

        if normalized.is_empty() {
            changed = true;
            continue;
        }

        if !seen.insert(normalized.clone()) {
            changed = true;
            continue;
        }

        normalized_parts.push(normalized);
    }

    if !changed {
        return Cow::Borrowed(filename);
    }

    normalized_parts.reverse();
    normalized_parts.insert(0, stem.to_owned());
    Cow::Owned(normalized_parts.join("."))
}

#[cfg(test)]
mod tests {
    use super::normalize_extension;
    use std::borrow::Cow;

    use pretty_assertions::assert_eq;

    #[test]
    fn keeps_clean_filename_without_extension() {
        assert!(matches!(
            normalize_extension("archive"),
            Cow::Borrowed("archive")
        ));
    }

    #[test]
    fn keeps_hidden_file_unchanged() {
        assert_eq!(normalize_extension(".env"), ".env");
    }

    #[test]
    fn removes_non_ascii_noise_from_extension_chain() {
        assert_eq!(normalize_extension("test.删去txt.删zip除"), "test.txt.zip");
    }

    #[test]
    fn deduplicates_repeated_extensions_without_other_changes() {
        assert_eq!(normalize_extension("archive.zip.zip"), "archive.zip");
    }

    #[test]
    fn drops_empty_segments_created_by_noise() {
        assert_eq!(normalize_extension("archive.删去"), "archive");
    }

    #[test]
    fn normalizes_extension_case_while_preserving_stem() {
        assert_eq!(normalize_extension("Photo.TAR.GZ"), "Photo.tar.gz");
    }
}
