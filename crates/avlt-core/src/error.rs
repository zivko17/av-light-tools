//! Error type shared by every avlt crate; each variant maps to a process exit code.

/// Errors returned by avlt-core and avlt-platform.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Bad user input (exit 2).
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// A required external tool is not installed (exit 3).
    #[error("'{tool}' not found. Install with: {hint}")]
    MissingTool { tool: String, hint: String },
    /// The action requires an elevated process (exit 4).
    #[error("administrator rights required")]
    NeedsAdmin,
    /// The action only exists on Windows (exit 5).
    #[error("not supported on this platform")]
    Unsupported,
    /// Anything else (exit 1).
    #[error("{0}")]
    Other(String),
    /// I/O failure (exit 1).
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Result alias used across avlt.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Process exit code for this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::InvalidInput(_) => 2,
            Error::MissingTool { .. } => 3,
            Error::NeedsAdmin => 4,
            Error::Unsupported => 5,
            Error::Other(_) | Error::Io(_) => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_follow_enum_order() {
        let errors = [
            Error::InvalidInput("x".into()),
            Error::MissingTool {
                tool: "ffmpeg".into(),
                hint: "winget install ffmpeg".into(),
            },
            Error::NeedsAdmin,
            Error::Unsupported,
            Error::Other("x".into()),
            Error::Io(std::io::Error::other("x")),
        ];
        let codes: Vec<i32> = errors.iter().map(Error::exit_code).collect();
        assert_eq!(codes, [2, 3, 4, 5, 1, 1]);
    }

    #[test]
    fn missing_tool_message_has_install_hint() {
        let e = Error::MissingTool {
            tool: "ffmpeg".into(),
            hint: "winget install Gyan.FFmpeg".into(),
        };
        assert_eq!(
            e.to_string(),
            "'ffmpeg' not found. Install with: winget install Gyan.FFmpeg"
        );
    }
}
