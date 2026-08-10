use std::path::PathBuf;

use crate::LanguageStr;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No ___.g.xlf file found in listed files")]
    NoBaseTranslationFound,
    #[error("No file name in path {0}")]
    InvalidFilePath(PathBuf),
    #[error("File name in path {0} needs to be utf8 or ascii encoded")]
    OnlyUtf8EncodedFilePathsAllowed(PathBuf),
    #[error("File path {0} did not match pattern of ___.[language].xlf")]
    UnsupportedFileNameFormat(PathBuf),
    #[error("Could not parse language of translation file: {0}")]
    LanguageStringToBig(#[from] tinystr::ParseError),
    #[error("Failed reading file because of io error. {0}")]
    IoError(#[from] std::io::Error),
    #[error("Parsing error: {0}")]
    ParsingError(#[from] quick_xml::de::DeError),
    #[error("Parsing error in file {0}: {1}")]
    ParsingErrorInFile(PathBuf, quick_xml::DeError),
    #[error("Serializing error: {0}")]
    SerializingError(#[from] quick_xml::se::SeError),
    #[error("Language {0} does not exist right now.")]
    LanguageNotFound(LanguageStr),
}
