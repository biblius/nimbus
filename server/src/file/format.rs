use serde::Serialize;
use sqlx::{Database, Decode, Encode, Type, encode::IsNull, error::BoxDynError};
use std::sync::OnceLock;

use crate::db::Db;

/// How many leading bytes [`FileType::infer`] keeps. Matches the `infer` crate's read limit.
pub const SNIFF_LEN: usize = 8192;

/// A file's family and format. The outer variant is the family.
///
/// Built-in variants are the types `infer` recognizes. Extra byte matchers are registered on the
/// `infer::Infer` inside [`FileType::infer`]. Anything that does not map to a variant is
/// [`FileType::Other`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    App(AppFormat),
    Book(BookFormat),
    Image(ImageFormat),
    Video(VideoFormat),
    Audio(AudioFormat),
    Font(FontFormat),
    Doc(DocFormat),
    Archive(ArchiveFormat),
    Text(TextFormat),
    Other(String),
}

macro_rules! formats {
    (
        $family:ident,
        $name:ident {
            $(
                $variant:ident => $ext:literal $(| $alias:literal)* , $mime:literal
            ),*
            $(,)?
        }
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
        pub enum $name {
            $(
                $variant
            ),*
        }

        impl $name {
            pub fn ext(self) -> &'static str {
                match self {
                    $(Self::$variant => $ext),*
                }
            }

            pub fn mime(self) -> &'static str {
                match self {
                    $(Self::$variant => $mime),*
                }
            }

            fn from_ext(ext: &str) -> Option<Self> {
                match ext {
                    $(
                        $ext $(| $alias )* => Some(Self::$variant),
                    )*
                    _ => None,
                }
            }
        }
    };
}

formats!(App, AppFormat {
    Wasm => "wasm", "application/wasm",
    Elf => "elf", "application/x-executable",
    Exe => "exe", "application/vnd.microsoft.portable-executable",
    Dll => "dll", "application/vnd.microsoft.portable-executable",
    Class => "class", "application/java",
    Bc => "bc", "application/x-llvm",
    Mach => "mach", "application/x-mach-binary",
    Dex => "dex", "application/vnd.android.dex",
    Dey => "dey", "application/vnd.android.dey",
    Der => "der", "application/x-x509-ca-cert",
    Obj => "obj", "application/x-executable",
    Pem => "pem", "application/x-x509-ca-cert",
    Qcow2 => "qcow2", "application/x-qemu-disk",
});

formats!(Book, BookFormat {
    Epub => "epub", "application/epub+zip",
    Mobi => "mobi", "application/x-mobipocket-ebook",
});

formats!(Image, ImageFormat {
    Jpg => "jpg" | "jpeg", "image/jpeg",
    Jp2 => "jp2", "image/jp2",
    Png => "png", "image/png",
    Gif => "gif", "image/gif",
    Webp => "webp", "image/webp",
    Cr2 => "cr2", "image/x-canon-cr2",
    Tif => "tif" | "tiff", "image/tiff",
    Bmp => "bmp", "image/bmp",
    Jxr => "jxr", "image/vnd.ms-photo",
    Psd => "psd", "image/vnd.adobe.photoshop",
    Ico => "ico", "image/vnd.microsoft.icon",
    Heif => "heif" | "heic", "image/heif",
    Avif => "avif", "image/avif",
    Jxl => "jxl", "image/jxl",
    Ora => "ora", "image/openraster",
    Djvu => "djvu", "image/vnd.djvu",
    Dwg => "dwg", "image/vnd.dwg",
});

formats!(Video, VideoFormat {
    Mp4 => "mp4", "video/mp4",
    M4v => "m4v", "video/x-m4v",
    Mkv => "mkv", "video/x-matroska",
    Webm => "webm", "video/webm",
    Mov => "mov", "video/quicktime",
    Avi => "avi", "video/x-msvideo",
    Wmv => "wmv", "video/x-ms-wmv",
    Mpg => "mpg" | "mpeg", "video/mpeg",
    Flv => "flv", "video/x-flv",
});

formats!(Audio, AudioFormat {
    Midi => "midi" | "mid", "audio/midi",
    Mp3 => "mp3", "audio/mpeg",
    M4a => "m4a", "audio/m4a",
    Opus => "opus", "audio/opus",
    Ogg => "ogg", "audio/ogg",
    Flac => "flac", "audio/x-flac",
    Wav => "wav", "audio/x-wav",
    Amr => "amr", "audio/amr",
    Aac => "aac", "audio/aac",
    Aiff => "aiff" |  "aif", "audio/x-aiff",
    Dsf => "dsf", "audio/x-dsf",
    Ape => "ape", "audio/x-ape",
});

formats!(Font, FontFormat {
    Woff => "woff", "application/font-woff",
    Woff2 => "woff2", "application/font-woff",
    Ttf => "ttf", "application/font-sfnt",
    Otf => "otf", "application/font-sfnt",
});

formats!(Doc, DocFormat {
    Pdf => "pdf", "application/pdf",
    Doc => "doc", "application/msword",
    Docx => "docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    Xls => "xls", "application/vnd.ms-excel",
    Xlsx => "xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    Ppt => "ppt", "application/vnd.ms-powerpoint",
    Pptx => "pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    Odt => "odt", "application/vnd.oasis.opendocument.text",
    Ods => "ods", "application/vnd.oasis.opendocument.spreadsheet",
    Odp => "odp", "application/vnd.oasis.opendocument.presentation",
});

formats!(Archive, ArchiveFormat {
    Zip => "zip", "application/zip",
    Tar => "tar", "application/x-tar",
    Par2 => "par2", "application/x-par2",
    Rar => "rar", "application/vnd.rar",
    Gz => "gz" | "gzip", "application/gzip",
    Bz2 => "bz2", "application/x-bzip2",
    Bz3 => "bz3", "application/vnd.bzip3",
    SevenZ => "7z", "application/x-7z-compressed",
    Xz => "xz", "application/x-xz",
    Swf => "swf", "application/x-shockwave-flash",
    Rtf => "rtf", "application/rtf",
    Eot => "eot", "application/octet-stream",
    Ps => "ps", "application/postscript",
    Sqlite => "sqlite", "application/vnd.sqlite3",
    Nes => "nes", "application/x-nintendo-nes-rom",
    Crx => "crx", "application/x-google-chrome-extension",
    Cab => "cab", "application/vnd.ms-cab-compressed",
    Deb => "deb", "application/vnd.debian.binary-package",
    Ar => "ar", "application/x-unix-archive",
    Z => "z", "application/x-compress",
    Lz => "lz", "application/x-lzip",
    Rpm => "rpm", "application/x-rpm",
    Dcm => "dcm", "application/dicom",
    Zst => "zst", "application/zstd",
    Lz4 => "lz4", "application/x-lz4",
    Msi => "msi", "application/x-ole-storage",
    Cpio => "cpio", "application/x-cpio",
});

formats!(Text, TextFormat {
    Html => "html" | "htm", "text/html",
    Xml => "xml", "text/xml",
    Sh => "sh" | "bash" | "zsh" | "fish", "text/x-shellscript",
    Txt => "txt" | "text", "text/plain; charset=utf-8",
    Json => "json", "application/json",
    Jsonc => "jsonc" | "json5", "application/json",
    Md => "md" | "markdown", "text/markdown; charset=utf-8",
    Csv => "csv", "text/csv; charset=utf-8",
    Tsv => "tsv", "text/tab-separated-values; charset=utf-8",
    Css => "css", "text/css; charset=utf-8",
    Yaml => "yaml" | "yml", "application/yaml",
    Toml => "toml", "application/toml",
    Ini => "ini", "text/plain; charset=utf-8",
    Conf => "conf", "text/plain; charset=utf-8",
    Env => "env", "text/plain; charset=utf-8",
    Properties => "properties", "text/x-java-properties",
    Log => "log", "text/plain; charset=utf-8",
    Sql => "sql", "application/sql",
    Graphql => "graphql" | "gql", "application/graphql",
    Rst => "rst", "text/x-rst",
    Tex => "tex" | "latex", "application/x-tex",
    Js => "js" | "mjs" | "cjs", "text/javascript; charset=utf-8",
    Jsx => "jsx", "text/jsx; charset=utf-8",
    Ts => "ts" | "mts" | "cts", "text/typescript; charset=utf-8",
    Tsx => "tsx", "text/tsx; charset=utf-8",
    Py => "py" | "pyi", "text/x-python; charset=utf-8",
    Rs => "rs", "text/x-rust; charset=utf-8",
    Go => "go", "text/x-go; charset=utf-8",
    Java => "java", "text/x-java-source; charset=utf-8",
    C => "c" | "h", "text/x-c; charset=utf-8",
    Cpp => "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx", "text/x-c++src; charset=utf-8",
    Rb => "rb", "text/x-ruby; charset=utf-8",
    Php => "php", "application/x-httpd-php",
    Lua => "lua", "text/x-lua; charset=utf-8",
    Kotlin => "kt" | "kts", "text/x-kotlin; charset=utf-8",
    Swift => "swift", "text/x-swift; charset=utf-8",
    Dart => "dart", "text/x-dart; charset=utf-8",
    Vue => "vue", "text/x-vue; charset=utf-8",
    Svelte => "svelte", "text/x-svelte; charset=utf-8",
});

impl FileType {
    /// Decide the file type from the leading bytes and the filename extension.
    ///
    /// Magic bytes and the extension must name the same format. When the bytes match nothing,
    /// the extension's format is used. A name with no extension and no magic bytes is plain text.
    pub fn infer(name: &str, bytes: &[u8]) -> Self {
        static INFER: OnceLock<infer::Infer> = OnceLock::new();

        let from_bytes = INFER
            .get_or_init(infer::Infer::new)
            .get(bytes)
            .map(|kind| Self::from_extension(kind.extension()));

        let ext = name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase());

        let from_ext = ext.as_deref().map(Self::from_extension);

        match (from_bytes, from_ext) {
            // OOXML / ODF / epub / ora are ZIP containers; prefer the specific extension.
            (
                Some(Self::Archive(ArchiveFormat::Zip)),
                Some(
                    ext_type @ (Self::Doc(
                        DocFormat::Docx
                        | DocFormat::Xlsx
                        | DocFormat::Pptx
                        | DocFormat::Odt
                        | DocFormat::Ods
                        | DocFormat::Odp,
                    )
                    | Self::Book(BookFormat::Epub)
                    | Self::Image(ImageFormat::Ora)),
                ),
            ) => ext_type,
            // Types infered from bytes always have priority.
            (Some(byte_type), _) => byte_type,
            // Types inferred from extensions must either be for text
            // files or unhandled files. Text files cannot be inferred
            // from bytes.
            (None, Some(ext_type)) => match ext_type {
                FileType::Text(_) | FileType::Other(_) => ext_type.clone(),
                // If this is not a text file or unhandled file we must flag it as
                // an unhandled file by setting it to other.
                _ => {
                    tracing::warn!(
                        "file type from bytes does not match extension: {:?}",
                        ext_type
                    );
                    Self::Other("other".into())
                }
            },
            // If there is no extension nor byte info, we default to unhandled.
            (None, None) => match ext {
                Some(ext) => Self::Other(ext),
                None => Self::Other("other".into()),
            },
        }
    }

    pub fn mime(&self) -> &'static str {
        match self {
            Self::App(format) => format.mime(),
            Self::Book(format) => format.mime(),
            Self::Image(format) => format.mime(),
            Self::Video(format) => format.mime(),
            Self::Audio(format) => format.mime(),
            Self::Font(format) => format.mime(),
            Self::Doc(format) => format.mime(),
            Self::Archive(format) => format.mime(),
            Self::Text(format) => format.mime(),
            Self::Other(_) => "application/octet-stream",
        }
    }

    /// Canonical extension stored in the `format` column.
    pub fn format(&self) -> &str {
        match self {
            Self::App(format) => format.ext(),
            Self::Book(format) => format.ext(),
            Self::Image(format) => format.ext(),
            Self::Video(format) => format.ext(),
            Self::Audio(format) => format.ext(),
            Self::Font(format) => format.ext(),
            Self::Doc(format) => format.ext(),
            Self::Archive(format) => format.ext(),
            Self::Text(format) => format.ext(),
            Self::Other(ext) => ext,
        }
    }

    pub fn from_extension(ext: &str) -> Self {
        let ext = ext.to_ascii_lowercase();
        AppFormat::from_ext(&ext)
            .map(Self::App)
            .or_else(|| BookFormat::from_ext(&ext).map(Self::Book))
            .or_else(|| ImageFormat::from_ext(&ext).map(Self::Image))
            .or_else(|| VideoFormat::from_ext(&ext).map(Self::Video))
            .or_else(|| AudioFormat::from_ext(&ext).map(Self::Audio))
            .or_else(|| FontFormat::from_ext(&ext).map(Self::Font))
            .or_else(|| DocFormat::from_ext(&ext).map(Self::Doc))
            .or_else(|| ArchiveFormat::from_ext(&ext).map(Self::Archive))
            .or_else(|| TextFormat::from_ext(&ext).map(Self::Text))
            .unwrap_or_else(|| Self::Other(ext.to_string()))
    }
}

impl Type<Db> for FileType {
    fn type_info() -> <Db as Database>::TypeInfo {
        <str as Type<Db>>::type_info()
    }

    fn compatible(ty: &<Db as Database>::TypeInfo) -> bool {
        <str as Type<Db>>::compatible(ty)
    }
}

impl<'q> Encode<'q, Db> for FileType {
    fn encode_by_ref(
        &self,
        buf: &mut <Db as Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
        <&str as Encode<'q, Db>>::encode(self.format(), buf)
    }
}

impl<'r> Decode<'r, Db> for FileType {
    fn decode(value: <Db as Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(Self::from_extension(<&str as Decode<'r, Db>>::decode(
            value,
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0x00];
    /// Minimal local-file header — enough for `infer` to call it a zip.
    const ZIP: &[u8] = &[0x50, 0x4B, 0x03, 0x04, 0x00, 0x00, 0x00, 0x00];

    #[test]
    fn zip_container_extensions_prefer_the_specific_format() {
        assert_eq!(
            FileType::infer("report.docx", ZIP),
            FileType::Doc(DocFormat::Docx)
        );
        assert_eq!(
            FileType::infer("sheet.xlsx", ZIP),
            FileType::Doc(DocFormat::Xlsx)
        );
        assert_eq!(
            FileType::infer("deck.pptx", ZIP),
            FileType::Doc(DocFormat::Pptx)
        );
        assert_eq!(
            FileType::infer("notes.odt", ZIP),
            FileType::Doc(DocFormat::Odt)
        );
        assert_eq!(
            FileType::infer("book.epub", ZIP),
            FileType::Book(BookFormat::Epub)
        );
        assert_eq!(
            FileType::infer("paint.ora", ZIP),
            FileType::Image(ImageFormat::Ora)
        );
    }

    #[test]
    fn plain_zip_stays_an_archive() {
        assert_eq!(
            FileType::infer("archive.zip", ZIP),
            FileType::Archive(ArchiveFormat::Zip)
        );
    }

    #[test]
    fn agreeing_magic_and_extension_keep_the_format() {
        let kind = FileType::infer("photo.png", PNG);
        assert_eq!(kind, FileType::Image(ImageFormat::Png));
        assert_eq!(kind.mime(), "image/png");
        assert_eq!(kind.format(), "png");
        assert_eq!(FileType::from_extension("png"), kind);
        assert_eq!(
            serde_json::to_value(&kind).unwrap(),
            serde_json::json!({ "image": "Png" })
        );
    }

    #[test]
    fn jpeg_alias_matches_magic() {
        assert_eq!(
            FileType::infer("photo.jpeg", JPEG),
            FileType::Image(ImageFormat::Jpg)
        );
    }

    #[test]
    fn bytes_take_priority_over_the_extension() {
        let kind = FileType::infer("photo.jpg", PNG);
        assert_eq!(kind, FileType::Image(ImageFormat::Png));
        assert_eq!(kind.format(), "png");
    }

    #[test]
    fn magic_without_an_extension_uses_the_bytes() {
        assert_eq!(
            FileType::infer("upload", JPEG),
            FileType::Image(ImageFormat::Jpg)
        );
    }

    #[test]
    fn missing_magic_uses_the_extension() {
        assert_eq!(
            FileType::infer("notes.json", br#"{"a":1}"#),
            FileType::Text(TextFormat::Json)
        );
    }

    #[test]
    fn yaml_and_toml_extensions_are_text() {
        assert_eq!(
            FileType::infer("config.yaml", b"key: value\n"),
            FileType::Text(TextFormat::Yaml)
        );
        assert_eq!(
            FileType::infer("config.yml", b"key: value\n"),
            FileType::Text(TextFormat::Yaml)
        );
        assert_eq!(
            FileType::infer("Cargo.toml", b"[package]\n"),
            FileType::Text(TextFormat::Toml)
        );
    }

    #[test]
    fn missing_magic_and_extension_is_other() {
        let kind = FileType::infer("README", b"hello");
        assert_eq!(kind, FileType::Other("other".into()));
        assert_eq!(FileType::from_extension(kind.format()), kind);
    }

    #[test]
    fn unknown_extension_without_magic_is_other() {
        assert_eq!(
            FileType::infer("data.bin", b"hello"),
            FileType::Other("bin".into())
        );
    }
}
