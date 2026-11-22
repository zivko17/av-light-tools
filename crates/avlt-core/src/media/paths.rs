//! Output paths that never overwrite an existing file.

use std::path::{Path, PathBuf};

/// `path` if nothing exists there, otherwise the first free `stem_1.ext`, `stem_2.ext`, ...
///
/// Only checks what exists now; writers should still open with `create_new` so a file
/// created in between is never clobbered.
pub fn non_colliding(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path.extension().map(|e| e.to_string_lossy());
    (1u64..)
        .map(|n| {
            let name = match &ext {
                Some(ext) => format!("{stem}_{n}.{ext}"),
                None => format!("{stem}_{n}"),
            };
            path.with_file_name(name)
        })
        .find(|p| !p.exists())
        .unwrap_or_else(|| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_path_is_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.mp3");
        assert_eq!(non_colliding(&p), p);
    }

    #[test]
    fn existing_file_gets_suffix_1() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.mp3"), b"x").unwrap();
        assert_eq!(
            non_colliding(&dir.path().join("a.mp3")),
            dir.path().join("a_1.mp3")
        );
    }

    #[test]
    fn existing_suffix_1_gets_suffix_2() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.mp3"), b"x").unwrap();
        std::fs::write(dir.path().join("a_1.mp3"), b"x").unwrap();
        assert_eq!(
            non_colliding(&dir.path().join("a.mp3")),
            dir.path().join("a_2.mp3")
        );
    }

    #[test]
    fn no_extension_and_non_ascii_names() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("carpeta con espacios é");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("señal"), b"x").unwrap();
        assert_eq!(non_colliding(&sub.join("señal")), sub.join("señal_1"));
        std::fs::write(sub.join("vídeo.bin"), b"x").unwrap();
        assert_eq!(
            non_colliding(&sub.join("vídeo.bin")),
            sub.join("vídeo_1.bin")
        );
    }
}
