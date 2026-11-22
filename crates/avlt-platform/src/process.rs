//! Locating and running external programs (netsh, ipconfig, speedtest, ffmpeg...).

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
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

/// A child process tied to a Windows Job Object with `KILL_ON_JOB_CLOSE`: when this
/// value is dropped, or when avlt exits by any path (window closed, Ctrl+C, crash), the
/// job handle closes and Windows kills the child. Elsewhere it is a plain child.
pub struct KillableChild {
    pub child: Child,
    _job: imp::Job,
}

impl KillableChild {
    /// Spawns `cmd` and puts the new process in its own kill-on-close job.
    pub fn spawn(cmd: &mut Command) -> Result<KillableChild> {
        let child = cmd.spawn()?;
        let job = imp::Job::for_child(&child);
        Ok(KillableChild { child, _job: job })
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }
}

/// [`output`] that kills the program as soon as `cancel` is set (the GUI's Cancel), then
/// fails with `Error::Other("cancelled")`. The program also dies with avlt
/// ([`KillableChild`]), so closing the window never leaves ffmpeg / speedtest running.
pub fn output_cancellable(exe: &Path, args: &[&str], cancel: &AtomicBool) -> Result<(i32, String)> {
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_window(&mut cmd);
    let mut killable = KillableChild::spawn(&mut cmd)?;
    let child = &mut killable.child;
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
mod imp {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;

    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows::core::PCWSTR;

    /// An owned job handle (`None` if the job could not be set up: the child then just
    /// runs unconfined, as before).
    pub struct Job(Option<HANDLE>);

    impl Job {
        pub fn for_child(child: &Child) -> Job {
            // SAFETY: no security attributes and no name; the returned handle is owned
            // by `Job` and closed exactly once in `Drop`.
            let Ok(job) = (unsafe { CreateJobObjectW(None, PCWSTR::null()) }) else {
                return Job(None);
            };
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            // SAFETY: `info` is a JOBOBJECT_EXTENDED_LIMIT_INFORMATION of the size passed;
            // the child's handle is valid while `child` is alive (it is, we borrow it).
            let ok = unsafe {
                SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    (&raw const info).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
                .is_ok()
                    && AssignProcessToJobObject(job, HANDLE(child.as_raw_handle())).is_ok()
            };
            if ok {
                Job(Some(job))
            } else {
                // SAFETY: `job` was created above and is not used afterwards.
                let _ = unsafe { CloseHandle(job) };
                Job(None)
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            if let Some(h) = self.0.take() {
                // SAFETY: owned handle, closed once; closing it kills the job's processes.
                let _ = unsafe { CloseHandle(h) };
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::process::Child;

    /// No job objects outside Windows.
    pub struct Job;

    impl Job {
        pub fn for_child(_child: &Child) -> Job {
            Job
        }
    }
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

    #[cfg(windows)]
    fn process_exists(pid: u32) -> bool {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
    }

    #[cfg(windows)]
    #[test]
    fn a_killable_child_dies_when_dropped() {
        let ping = find_tool("ping", "none").unwrap();
        let mut cmd = Command::new(ping);
        cmd.args(["-n", "30", "127.0.0.1"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let child = KillableChild::spawn(&mut cmd).unwrap();
        let pid = child.id();
        assert!(process_exists(pid), "ping {pid} should be running");
        drop(child);
        let end = std::time::Instant::now() + Duration::from_secs(5);
        while process_exists(pid) && std::time::Instant::now() < end {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!process_exists(pid), "ping {pid} survived its job handle");
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
