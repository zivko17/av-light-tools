//! Output paths that never overwrite an existing file.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::Result;

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

/// Writes `bytes` to `path` or the first free `_1`, `_2`, ... name; never overwrites.
/// Returns the path written.
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<PathBuf> {
    loop {
        let target = non_colliding(path);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut f) => {
                f.write_all(bytes)?;
                return Ok(target);
            }
            // Created by someone else between the check and the open: try the next name.
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }
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

    #[test]
    fn write_new_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("é d.bin");
        assert_eq!(write_new(&p, b"a").unwrap(), p);
        let second = write_new(&p, b"b").unwrap();
        assert_eq!(second, dir.path().join("é d_1.bin"));
        assert_eq!(std::fs::read(&p).unwrap(), b"a");
        assert_eq!(std::fs::read(&second).unwrap(), b"b");
    }
}
