//! Shared environment lookups: the executable's folder, `avlt.toml`, the output folder.

use std::path::PathBuf;

use avlt_core::Result;
use avlt_core::config::{Config, is_portable};
use avlt_platform::paths::{KnownFolder, known_folder};

/// The folder holding the running executable (`.` when it cannot be determined).
pub(crate) fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `avlt.toml` next to the executable (defaults when absent).
pub(crate) fn load_config() -> Result<Config> {
    Config::load(&exe_dir())
}

/// Where avlt writes files: `avlt.toml` / portable mode, else `<folder>\AVLT`.
pub(crate) fn output_dir(folder: KnownFolder) -> Result<PathBuf> {
    let dir = exe_dir();
    let cfg = Config::load(&dir)?;
    Ok(cfg.output_dir(&dir, is_portable(&dir), known_folder(folder).as_deref()))
}
