//! Locating and running external programs (netsh, ipconfig, speedtest, ffmpeg...).

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::{Error, Result};

/// Finds `name` next to the avlt executable or on `PATH` (`name.exe` on Windows).
///
/// Fails with [`Error::MissingTool`] whose message suggests `winget install <winget_id>`.
pub fn find_tool(name: &str, winget_id: &str) -> Result<PathBuf> {
    let file = file_name(name);
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let path_dirs = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default();
    exe_dir
        .into_iter()
        .chain(path_dirs)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(&file))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| Error::MissingTool {
            tool: name.to_string(),
            hint: format!("winget install {winget_id}"),
        })
}

fn file_name(name: &str) -> OsString {
    let has_exe = Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
    if cfg!(windows) && !has_exe {
        format!("{name}.exe").into()
    } else {
        name.into()
    }
}

/// Runs `exe` with the console's stdin/stdout/stderr and waits for it.
pub fn run(exe: &Path, args: &[&str]) -> Result<ExitStatus> {
    Ok(Command::new(exe).args(args).status()?)
}

/// Runs `exe` without a window and returns its exit code and stdout followed by stderr.
/// The exit code is -1 if the process was killed by a signal.
pub fn output(exe: &Path, args: &[&str]) -> Result<(i32, String)> {
    let out = hidden_output(exe, args)?;
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok((out.status.code().unwrap_or(-1), text))
}

/// [`output`] that kills the program as soon as `cancel` is set (the GUI's Cancel), then
/// fails with `Error::Other("cancelled")`.
pub fn output_cancellable(exe: &Path, args: &[&str], cancel: &AtomicBool) -> Result<(i32, String)> {
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_window(&mut cmd);
    let mut child = cmd.spawn()?;
    // Both pipes are drained on their own threads so a chatty program never blocks.
    let read = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    };
    let out = read(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let err = read(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let status = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Other("cancelled".into()));
        }
        match child.try_wait()? {
            Some(status) => break status,
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    let mut text = String::from_utf8_lossy(&out.join().unwrap_or_default()).into_owned();
    text.push_str(&String::from_utf8_lossy(&err.join().unwrap_or_default()));
    Ok((status.code().unwrap_or(-1), text))
}

/// Runs `exe` without a window and returns its stdout.
///
/// A non-zero exit becomes [`Error::Other`] carrying the exit code and the program's output.
pub fn capture(exe: &Path, args: &[&str]) -> Result<String> {
    let out = hidden_output(exe, args)?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    if out.status.success() {
        return Ok(stdout);
    }
    let detail = format!("{}{}", stdout, String::from_utf8_lossy(&out.stderr));
    let name = exe.file_name().map_or_else(
        || exe.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    Err(Error::Other(match out.status.code() {
        Some(code) => format!("{name} failed with exit code {code}: {}", detail.trim()),
        None => format!("{name} was terminated: {}", detail.trim()),
    }))
}

/// Runs `exe` with no stdin and no console window, capturing stdout and stderr.
fn hidden_output(exe: &Path, args: &[&str]) -> Result<std::process::Output> {
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_window(&mut cmd);
    Ok(cmd.output()?)
}

#[cfg(windows)]
fn hide_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_window(_cmd: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tool_has_winget_hint() {
        let err = find_tool("definitely-not-a-tool", "X.Y").unwrap_err();
        assert!(matches!(err, Error::MissingTool { .. }), "{err:?}");
        assert_eq!(err.exit_code(), 3);
        assert!(err.to_string().contains("winget install X.Y"), "{err}");
        assert!(err.to_string().contains("definitely-not-a-tool"), "{err}");
    }

    #[cfg(windows)]
    const SHELL: (&str, &str) = ("cmd", "/c");
    #[cfg(not(windows))]
    const SHELL: (&str, &str) = ("sh", "-c");

    fn shell() -> PathBuf {
        find_tool(SHELL.0, "none").unwrap()
    }

    #[test]
    fn finds_a_tool_on_path() {
        let p = shell();
        assert!(p.is_file(), "{}", p.display());
    }

    #[test]
    fn capture_returns_stdout() {
        let out = capture(&shell(), &[SHELL.1, "echo hello"]).unwrap();
        assert_eq!(out.trim(), "hello");
    }

    #[test]
    fn capture_fails_on_nonzero_exit_with_output() {
        let err = capture(&shell(), &[SHELL.1, "echo boom && exit 3"]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("exit code 3"), "{msg}");
        assert!(msg.contains("boom"), "{msg}");
    }

    #[test]
    fn output_reports_code_and_text() {
        let (code, text) = output(&shell(), &[SHELL.1, "echo out && exit 2"]).unwrap();
        assert_eq!(code, 2);
        assert!(text.contains("out"), "{text}");
    }

    #[test]
    fn output_cancellable_runs_and_cancels() {
        let go = AtomicBool::new(false);
        let (code, text) =
            output_cancellable(&shell(), &[SHELL.1, "echo out && exit 2"], &go).unwrap();
        assert_eq!(code, 2);
        assert!(text.contains("out"), "{text}");
        let stop = AtomicBool::new(true);
        let err = output_cancellable(&shell(), &[SHELL.1, "echo x"], &stop).unwrap_err();
        assert_eq!(err.to_string(), "cancelled");
    }

    #[test]
    fn run_returns_status() {
        let status = run(&shell(), &[SHELL.1, "exit 0"]).unwrap();
        assert!(status.success());
        let status = run(&shell(), &[SHELL.1, "exit 4"]).unwrap();
        assert_eq!(status.code(), Some(4));
    }

    #[test]
    fn spawning_a_missing_file_is_io_error() {
        let err = capture(Path::new("definitely/not/here.exe"), &[]).unwrap_err();
        assert!(matches!(err, Error::Io(_)), "{err:?}");
    }
}
