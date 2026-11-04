use std::path::Path;

#[derive(Debug)]
pub enum Error {
    EmptyPassword,
    EmptySelection,
    NotAFile(String),
    Io { path: String, message: String },
    Archive(String),
    BadMagic,
    UnsupportedVersion(u8),
    Truncated,
    InvalidParams,
    Decrypt,
    UnsafePath(String),
}

impl Error {
    pub(crate) fn io(path: impl AsRef<Path>, err: std::io::Error) -> Self {
        Self::Io {
            path: path.as_ref().display().to_string(),
            message: err.to_string(),
        }
    }

    pub(crate) fn archive(err: impl std::fmt::Display) -> Self {
        Self::Archive(err.to_string())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPassword => write!(f, "Enter a password."),
            Self::EmptySelection => write!(f, "Choose at least one file."),
            Self::NotAFile(path) => write!(f, "{path} is not a file."),
            Self::Io { path, message } => write!(f, "Couldn't access {path}: {message}"),
            Self::Archive(message) => write!(f, "Couldn't read the zip payload: {message}"),
            Self::BadMagic => write!(f, "This file is not a .wzip archive."),
            Self::UnsupportedVersion(version) => {
                write!(f, "This .wzip version ({version}) is not supported.")
            }
            Self::Truncated => write!(f, "This .wzip archive is incomplete."),
            Self::InvalidParams => {
                write!(f, "This .wzip archive uses unsupported key parameters.")
            }
            Self::Decrypt => write!(
                f,
                "Couldn't decrypt the archive. The password may be wrong, or the file is damaged."
            ),
            Self::UnsafePath(path) => {
                write!(f, "The archive contains an unsafe path ({path}).")
            }
        }
    }
}

impl std::error::Error for Error {}
