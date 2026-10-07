use serde::Serialize;
use sqlx::{Database, Decode, Encode, Type, encode::IsNull, error::BoxDynError};
use std::fmt::Display;
use utoipa::ToSchema;

use crate::db::Db;

/// Top-level classification used to group files in the client and determine which type of metadata
/// and processing methods to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
pub enum FileFamily {
    /// Image files
    Image,
    /// Audio files
    Audio,
    /// Video files
    Video,
    /// Text based (unicode) files that can be directly parsed to strings.
    Text,
    /// Text based files with a specific structure, ultimately determined by the format.
    Doc,
    /// Unknown file family
    Unknown,
}

impl FileFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Text => "text",
            Self::Doc => "doc",
            Self::Unknown => "unknown",
        }
    }
}

impl From<&str> for FileFamily {
    fn from(value: &str) -> Self {
        match value {
            "image" => Self::Image,
            "audio" => Self::Audio,
            "video" => Self::Video,
            "text" => Self::Text,
            "doc" => Self::Doc,
            _ => Self::Unknown,
        }
    }
}

impl From<String> for FileFamily {
    fn from(value: String) -> Self {
        value.as_str().into()
    }
}

impl Display for FileFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Type<Db> for FileFamily {
    fn type_info() -> <Db as Database>::TypeInfo {
        <str as Type<Db>>::type_info()
    }

    fn compatible(ty: &<Db as Database>::TypeInfo) -> bool {
        <str as Type<Db>>::compatible(ty)
    }
}

impl<'q> Encode<'q, Db> for FileFamily {
    fn encode_by_ref(
        &self,
        buf: &mut <Db as Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
        <&'q str as Encode<'q, Db>>::encode_by_ref(&self.as_str(), buf)
    }
}

impl<'r> Decode<'r, Db> for FileFamily {
    fn decode(value: <Db as Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(Self::from(<&'r str as Decode<'r, Db>>::decode(value)?))
    }
}

/// File format. Unknown extensions use [`FileFormat::Other`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub enum FileFormat {
    Jpg,
    Png,
    Wav,
    Mp3,
    Avi,
    Mp4,
    Txt,
    Json,
    Html,
    Xml,
    Md,
    Csv,
    Css,
    Rtf,
    Doc,
    Docx,
    Odt,
    Ods,
    Odp,
    Pdf,
    Xls,
    Xlsx,
    Ppt,
    Pptx,
    Other(String),
}

impl FileFormat {
    pub fn as_str(&self) -> &str {
        match self {
            FileFormat::Jpg => "jpg",
            FileFormat::Png => "png",
            FileFormat::Wav => "wav",
            FileFormat::Mp3 => "mp3",
            FileFormat::Avi => "avi",
            FileFormat::Mp4 => "mp4",
            FileFormat::Txt => "txt",
            FileFormat::Json => "json",
            FileFormat::Html => "html",
            FileFormat::Xml => "xml",
            FileFormat::Md => "md",
            FileFormat::Csv => "csv",
            FileFormat::Css => "css",
            FileFormat::Rtf => "rtf",
            FileFormat::Doc => "doc",
            FileFormat::Docx => "docx",
            FileFormat::Odt => "odt",
            FileFormat::Ods => "ods",
            FileFormat::Odp => "odp",
            FileFormat::Pdf => "pdf",
            FileFormat::Xls => "xls",
            FileFormat::Xlsx => "xlsx",
            FileFormat::Ppt => "ppt",
            FileFormat::Pptx => "pptx",
            FileFormat::Other(ext) => ext,
        }
    }
}

impl From<&str> for FileFormat {
    fn from(value: &str) -> Self {
        match value {
            "jpg" | "jpeg" => Self::Jpg,
            "png" => Self::Png,
            "wav" => Self::Wav,
            "mp3" => Self::Mp3,
            "avi" => Self::Avi,
            "mp4" => Self::Mp4,
            "txt" => Self::Txt,
            "json" => Self::Json,
            "html" | "htm" => Self::Html,
            "xml" => Self::Xml,
            "md" | "markdown" => Self::Md,
            "csv" => Self::Csv,
            "css" => Self::Css,
            "rtf" => Self::Rtf,
            "doc" => Self::Doc,
            "docx" => Self::Docx,
            "odt" => Self::Odt,
            "ods" => Self::Ods,
            "odp" => Self::Odp,
            "pdf" => Self::Pdf,
            "xls" => Self::Xls,
            "xlsx" => Self::Xlsx,
            "ppt" => Self::Ppt,
            "pptx" => Self::Pptx,
            ext => Self::Other(ext.to_string()),
        }
    }
}

impl From<String> for FileFormat {
    fn from(value: String) -> Self {
        value.as_str().into()
    }
}

impl Display for FileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FileFormat {
    pub fn from_ext(name: &str) -> Self {
        let Some((_, ext)) = name.rsplit_once('.') else {
            return Self::Other("unknown".into());
        };

        if ext.is_empty() || ext.len() > 32 {
            return Self::Other("unknown".into());
        }

        Self::from(ext.to_ascii_lowercase().as_str())
    }

    pub fn family(&self) -> FileFamily {
        match self {
            Self::Jpg | Self::Png => FileFamily::Image,
            Self::Wav | Self::Mp3 => FileFamily::Audio,
            Self::Avi | Self::Mp4 => FileFamily::Video,
            Self::Txt | Self::Json | Self::Html | Self::Xml | Self::Md | Self::Csv | Self::Css => {
                FileFamily::Text
            }
            Self::Rtf
            | Self::Doc
            | Self::Docx
            | Self::Odt
            | Self::Ods
            | Self::Odp
            | Self::Pdf
            | Self::Xls
            | Self::Xlsx
            | Self::Ppt
            | Self::Pptx => FileFamily::Doc,
            Self::Other(ext) => match ext.as_str() {
                "gif" | "webp" | "svg" | "bmp" | "heic" => FileFamily::Image,
                "flac" | "ogg" | "aac" | "m4a" => FileFamily::Audio,
                "mkv" | "webm" | "mov" => FileFamily::Video,
                "js" | "ts" | "rs" | "yaml" | "yml" | "toml" => FileFamily::Text,
                _ => FileFamily::Unknown,
            },
        }
    }

    pub fn mime(&self) -> &'static str {
        match self {
            Self::Jpg => "image/jpeg",
            Self::Png => "image/png",
            Self::Wav => "audio/wav",
            Self::Mp3 => "audio/mpeg",
            Self::Avi => "video/x-msvideo",
            Self::Mp4 => "video/mp4",
            Self::Txt => "text/plain; charset=utf-8",
            Self::Json => "application/json",
            Self::Html => "text/html; charset=utf-8",
            Self::Xml => "application/xml",
            Self::Md => "text/markdown; charset=utf-8",
            Self::Csv => "text/csv; charset=utf-8",
            Self::Css => "text/css; charset=utf-8",
            Self::Rtf => "application/rtf",
            Self::Doc => "application/msword",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Odt => "application/vnd.oasis.opendocument.text",
            Self::Ods => "application/vnd.oasis.opendocument.spreadsheet",
            Self::Odp => "application/vnd.oasis.opendocument.presentation",
            Self::Pdf => "application/pdf",
            Self::Xls => "application/vnd.ms-excel",
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Self::Ppt => "application/vnd.ms-powerpoint",
            Self::Pptx => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            }
            Self::Other(_) => "application/octet-stream",
        }
    }
}

impl Type<Db> for FileFormat {
    fn type_info() -> <Db as Database>::TypeInfo {
        <str as Type<Db>>::type_info()
    }

    fn compatible(ty: &<Db as Database>::TypeInfo) -> bool {
        <str as Type<Db>>::compatible(ty)
    }
}

impl<'q> Encode<'q, Db> for FileFormat {
    fn encode_by_ref(
        &self,
        buf: &mut <Db as Database>::ArgumentBuffer,
    ) -> Result<IsNull, BoxDynError> {
        <&str as Encode<'q, Db>>::encode_by_ref(&self.as_str(), buf)
    }
}

impl<'r> Decode<'r, Db> for FileFormat {
    fn decode(value: <Db as Database>::ValueRef<'r>) -> Result<Self, BoxDynError> {
        Ok(Self::from(<&str as Decode<'r, Db>>::decode(value)?))
    }
}
