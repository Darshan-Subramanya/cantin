//! File categorisation by extension and by well-known directory names.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Category {
    Video,
    Image,
    Audio,
    Document,
    Archive,
    Code,
    Installer,
    DiskImage,
    /// Dependency/build folders: node_modules, __pycache__, .venv, DerivedData, ...
    DevArtifacts,
    /// Caches and temp folders.
    Cache,
    Log,
    Other,
}

impl Category {
    pub const ALL: [Category; 12] = [
        Category::Video,
        Category::Image,
        Category::Audio,
        Category::Document,
        Category::Archive,
        Category::Code,
        Category::Installer,
        Category::DiskImage,
        Category::DevArtifacts,
        Category::Cache,
        Category::Log,
        Category::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::Video => "Videos",
            Category::Image => "Images",
            Category::Audio => "Audio",
            Category::Document => "Documents",
            Category::Archive => "Archives",
            Category::Code => "Code",
            Category::Installer => "Apps & installers",
            Category::DiskImage => "Disk & VM images",
            Category::DevArtifacts => "Dev dependencies & builds",
            Category::Cache => "Caches & temp",
            Category::Log => "Logs",
            Category::Other => "Other",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_str_loose(s: &str) -> Option<Category> {
        let s = s.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        Some(match s.as_str() {
            "video" | "videos" | "movie" | "movies" => Category::Video,
            "image" | "images" | "photo" | "photos" | "picture" | "pictures" => Category::Image,
            "audio" | "music" | "sound" | "songs" => Category::Audio,
            "document" | "documents" | "doc" | "docs" => Category::Document,
            "archive" | "archives" | "zip" | "compressed" => Category::Archive,
            "code" | "source" | "source_code" => Category::Code,
            "installer" | "installers" | "app" | "apps" | "application" | "applications" => {
                Category::Installer
            }
            "disk_image" | "disk_images" | "vm" | "vm_image" | "iso" => Category::DiskImage,
            "dev_artifacts" | "dev" | "dependencies" | "node_modules" | "build" => {
                Category::DevArtifacts
            }
            "cache" | "caches" | "temp" | "tmp" => Category::Cache,
            "log" | "logs" => Category::Log,
            "other" => Category::Other,
            _ => return None,
        })
    }
}

/// Directory names whose whole subtree belongs to a category.
pub fn dir_context(name: &str, inherited: Option<Category>) -> Option<Category> {
    if inherited.is_some() {
        return inherited;
    }
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "node_modules" | "__pycache__" | ".venv" | "venv" | ".gradle" | ".m2" | "deriveddata"
        | ".next" | ".nuxt" | ".tox" | ".pytest_cache" | ".mypy_cache" | "pods" | ".terraform"
        | ".parcel-cache" | ".turbo" => Some(Category::DevArtifacts),
        ".cache" | "caches" | "cache" | "temp" | "tmp" | "$recycle.bin" | ".trash" | "inetcache" => {
            Some(Category::Cache)
        }
        "logs" => Some(Category::Log),
        _ => None,
    }
}

/// macOS bundles are directories; treat them as a single app.
pub fn is_bundle_dir(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".app")
}

pub fn classify_file(name: &str, inherited: Option<Category>) -> Category {
    if let Some(c) = inherited {
        return c;
    }
    let ext = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => ext.to_ascii_lowercase(),
        _ => return Category::Other,
    };
    by_extension(&ext)
}

pub fn by_extension(ext: &str) -> Category {
    match ext {
        "mp4" | "mkv" | "mov" | "avi" | "wmv" | "flv" | "webm" | "m4v" | "mpg" | "mpeg" | "3gp"
        | "mts" | "m2ts" | "vob" => Category::Video,
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tif" | "tiff" | "webp" | "heic" | "heif"
        | "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng" | "psd" | "svg" | "ico" | "avif" => {
            Category::Image
        }
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" | "wma" | "aiff" | "aif" | "opus"
        | "alac" | "mid" | "midi" => Category::Audio,
        "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "odp"
        | "txt" | "rtf" | "md" | "csv" | "epub" | "pages" | "numbers" | "key" => {
            Category::Document
        }
        "zip" | "rar" | "7z" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "lz4" | "cab" => {
            Category::Archive
        }
        "rs" | "py" | "js" | "jsx" | "ts" | "tsx" | "java" | "c" | "h" | "cpp" | "hpp" | "cc" | "go"
        | "rb" | "php" | "swift" | "kt" | "cs" | "sh" | "json" | "yaml" | "yml" | "toml"
        | "html" | "css" | "scss" | "sql" | "lua" | "dart" | "scala" => Category::Code,
        "exe" | "msi" | "dmg" | "pkg" | "deb" | "rpm" | "appimage" | "apk" | "msix" | "snap"
        | "flatpak" => Category::Installer,
        "iso" | "img" | "vmdk" | "vdi" | "vhd" | "vhdx" | "qcow2" | "ova" | "ovf" | "sparseimage"
        | "sparsebundle" => Category::DiskImage,
        "log" => Category::Log,
        "tmp" | "temp" | "bak" | "swp" | "part" | "crdownload" => Category::Cache,
        _ => Category::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_extension() {
        assert_eq!(classify_file("Holiday.MP4", None), Category::Video);
        assert_eq!(classify_file("photo.heic", None), Category::Image);
        assert_eq!(classify_file("setup.exe", None), Category::Installer);
        assert_eq!(classify_file("Makefile", None), Category::Other);
        assert_eq!(classify_file(".bashrc", None), Category::Other);
    }

    #[test]
    fn dir_context_is_inherited() {
        let ctx = dir_context("node_modules", None);
        assert_eq!(ctx, Some(Category::DevArtifacts));
        assert_eq!(classify_file("big.mp4", ctx), Category::DevArtifacts);
        assert_eq!(dir_context("src", ctx), ctx);
    }

    #[test]
    fn loose_names() {
        assert_eq!(Category::from_str_loose("Videos"), Some(Category::Video));
        assert_eq!(Category::from_str_loose("disk image"), Some(Category::DiskImage));
        assert_eq!(Category::from_str_loose("bogus"), None);
    }
}
