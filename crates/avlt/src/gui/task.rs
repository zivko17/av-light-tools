//! Background work for the GUI.
//!
//! Long operations run on a Tokio runtime of their own (never on the UI thread) and post
//! messages over a channel; the UI drains the channel every frame. Every job can be
//! cancelled: the cancel flag is set (blocking work checks it), the async task is aborted
//! and the receiver is dropped, so late messages of a cancelled job are never seen.
//!
//! The state types here ([`Task`], [`Live`]) are plain structs with an `update(msg)`
//! function, so their transitions are tested without a window.

use std::collections::VecDeque;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;

use avlt_core::Result;
use eframe::egui;

/// Wakes the UI when a job posts a message (nothing in tests).
#[derive(Clone, Default)]
pub struct Repaint(Option<egui::Context>);

impl Repaint {
    pub fn new(ctx: egui::Context) -> Repaint {
        Repaint(Some(ctx))
    }

    fn request(&self) {
        if let Some(ctx) = &self.0 {
            ctx.request_repaint();
        }
    }
}

/// The sending half given to a job.
pub struct Sender<M> {
    tx: mpsc::Sender<M>,
    repaint: Repaint,
    cancel: Arc<AtomicBool>,
}

impl<M> Sender<M> {
    /// Posts a message; `false` once the job was cancelled or the UI dropped it, so a
    /// loop can stop.
    pub fn send(&self, msg: M) -> bool {
        let delivered = self.tx.send(msg).is_ok();
        self.repaint.request();
        delivered && !self.cancelled()
    }

    /// `true` after Cancel.
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// The cancel flag, for blocking platform functions that poll it.
    pub fn cancel_flag(&self) -> &AtomicBool {
        &self.cancel
    }
}

/// The UI's handle on a running job.
pub struct Job<M> {
    rx: mpsc::Receiver<M>,
    cancel: Arc<AtomicBool>,
    abort: Option<tokio::task::AbortHandle>,
}

impl<M> Job<M> {
    /// Stops the job: sets the cancel flag and aborts the async task.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Some(a) = &self.abort {
            a.abort();
        }
    }

    /// Every message posted since the last call.
    pub fn drain(&self) -> Vec<M> {
        self.rx.try_iter().collect()
    }
}

impl<M> Drop for Job<M> {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// The runtime that runs every job.
pub struct Runner {
    /// Taken on drop to shut down without waiting.
    rt: Option<tokio::runtime::Runtime>,
    handle: tokio::runtime::Handle,
    repaint: Repaint,
    /// Cancel flags of every job started, for [`Runner::cancel_all`] at exit.
    flags: std::sync::Mutex<Vec<std::sync::Weak<AtomicBool>>>,
}

/// How long closing the window waits for cancelled jobs to stop (and kill their
/// external programs) before the runtime is abandoned.
pub const EXIT_GRACE: std::time::Duration = std::time::Duration::from_millis(200);

impl Runner {
    pub fn new(repaint: Repaint) -> Result<Runner> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("avlt-gui-worker")
            .build()?;
        Ok(Runner {
            handle: rt.handle().clone(),
            rt: Some(rt),
            repaint,
            flags: std::sync::Mutex::new(Vec::new()),
        })
    }

    fn channel<M>(&self) -> (Sender<M>, mpsc::Receiver<M>, Arc<AtomicBool>) {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        if let Ok(mut flags) = self.flags.lock() {
            flags.retain(|w| w.strong_count() > 0);
            flags.push(Arc::downgrade(&cancel));
        }
        let sender = Sender {
            tx,
            repaint: self.repaint.clone(),
            cancel: Arc::clone(&cancel),
        };
        (sender, rx, cancel)
    }

    /// Runs an async job.
    pub fn spawn<M, F, Fut>(&self, job: F) -> Job<M>
    where
        M: Send + 'static,
        F: FnOnce(Sender<M>) -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (tx, rx, cancel) = self.channel();
        let handle = self.handle.spawn(job(tx));
        Job {
            rx,
            cancel,
            abort: Some(handle.abort_handle()),
        }
    }

    /// Runs blocking work (WMI, file system, external programs) on the blocking pool.
    pub fn spawn_blocking<M, F>(&self, job: F) -> Job<M>
    where
        M: Send + 'static,
        F: FnOnce(Sender<M>) + Send + 'static,
    {
        let (tx, rx, cancel) = self.channel();
        let handle = self.handle.spawn_blocking(move || job(tx));
        Job {
            rx,
            cancel,
            abort: Some(handle.abort_handle()),
        }
    }

    /// Runs `f` on the blocking pool and posts its result as [`Msg::Finished`].
    pub fn run<T, F>(&self, f: F) -> Job<Msg<T>>
    where
        T: Send + 'static,
        F: FnOnce(&Sender<Msg<T>>) -> Result<T> + Send + 'static,
    {
        self.spawn_blocking(move |tx| {
            let r = f(&tx).map_err(|e| e.to_string());
            tx.send(Msg::Finished(r));
        })
    }
}

impl Runner {
    /// Sets the cancel flag of every job still alive (blocking jobs see it and stop,
    /// killing ffmpeg / speedtest).
    pub fn cancel_all(&self) {
        if let Ok(flags) = self.flags.lock() {
            for f in flags.iter().filter_map(std::sync::Weak::upgrade) {
                f.store(true, Ordering::Relaxed);
            }
        }
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        // Cancel everything, give the jobs [`EXIT_GRACE`] to stop, then abandon what is
        // left (a slow WMI call must not delay closing the window). External programs
        // die with avlt anyway (kill-on-close job objects in avlt-platform).
        self.cancel_all();
        if let Some(rt) = self.rt.take() {
            rt.shutdown_timeout(EXIT_GRACE);
        }
    }
}

// ---------------------------------------------------------------- one-shot task

/// Where a one-shot operation stands.
#[derive(Debug, Clone, PartialEq)]
pub enum Status<T> {
    Idle,
    Running { done: u64, total: u64, note: String },
    Done(T),
    Failed(String),
}

/// What a one-shot job posts.
#[derive(Debug, Clone, PartialEq)]
pub enum Msg<T> {
    Progress { done: u64, total: u64, note: String },
    Finished(std::result::Result<T, String>),
}

/// A one-shot operation (scan, cleanup, speedtest...): state plus its running job.
pub struct Task<T> {
    pub status: Status<T>,
    job: Option<Job<Msg<T>>>,
}

impl<T> Default for Task<T> {
    fn default() -> Self {
        Task {
            status: Status::Idle,
            job: None,
        }
    }
}

impl<T> Task<T> {
    /// Idle -> Running with the job's messages feeding [`Task::update`].
    pub fn start(&mut self, job: Job<Msg<T>>) {
        self.status = Status::Running {
            done: 0,
            total: 0,
            note: String::new(),
        };
        self.job = Some(job);
    }

    /// Applies one message. Messages outside Running (a job cancelled meanwhile) are ignored.
    pub fn update(&mut self, msg: Msg<T>) {
        if !self.is_running() {
            return;
        }
        match msg {
            Msg::Progress { done, total, note } => {
                self.status = Status::Running { done, total, note };
            }
            Msg::Finished(r) => {
                self.status = match r {
                    Ok(v) => Status::Done(v),
                    Err(e) => Status::Failed(e),
                };
                self.job = None;
            }
        }
    }

    /// Cancel: Running -> Idle; the job is stopped and its later messages dropped.
    pub fn cancel(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel();
        }
        if self.is_running() {
            self.status = Status::Idle;
        }
    }

    /// Applies every message the job posted since the last frame.
    pub fn poll(&mut self) {
        let msgs = self.job.as_ref().map(Job::drain).unwrap_or_default();
        for m in msgs {
            self.update(m);
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self.status, Status::Running { .. })
    }

    /// The result, once done.
    pub fn done(&self) -> Option<&T> {
        match &self.status {
            Status::Done(v) => Some(v),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------- live monitor

/// Samples a live monitor keeps for its graph (two minutes at one per second).
pub const HISTORY: usize = 120;

/// What a live monitor's job posts.
#[derive(Debug, Clone, PartialEq)]
pub enum LiveMsg<S> {
    Sample(S),
    Failed(String),
}

/// A monitor that samples until stopped (ping, latency, traffic, telemetry, top).
pub struct Live<S> {
    pub samples: VecDeque<S>,
    pub error: Option<String>,
    job: Option<Job<LiveMsg<S>>>,
}

impl<S> Default for Live<S> {
    fn default() -> Self {
        Live {
            samples: VecDeque::new(),
            error: None,
            job: None,
        }
    }
}

impl<S> Live<S> {
    /// Starts sampling, clearing the previous run.
    pub fn start(&mut self, job: Job<LiveMsg<S>>) {
        self.samples.clear();
        self.error = None;
        self.job = Some(job);
    }

    pub fn update(&mut self, msg: LiveMsg<S>) {
        if !self.is_running() {
            return;
        }
        match msg {
            LiveMsg::Sample(s) => {
                self.samples.push_back(s);
                while self.samples.len() > HISTORY {
                    self.samples.pop_front();
                }
            }
            LiveMsg::Failed(e) => {
                self.error = Some(e);
                self.stop();
            }
        }
    }

    /// Stops sampling; the samples so far stay on screen.
    pub fn stop(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel();
        }
    }

    pub fn poll(&mut self) {
        let msgs = self.job.as_ref().map(Job::drain).unwrap_or_default();
        for m in msgs {
            self.update(m);
        }
    }

    pub fn is_running(&self) -> bool {
        self.job.is_some()
    }

    pub fn last(&self) -> Option<&S> {
        self.samples.back()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    pub(crate) fn runner() -> Runner {
        Runner::new(Repaint::default()).unwrap()
    }

    /// Polls `task` until it leaves Running (or 5 s pass).
    pub(crate) fn wait<T>(task: &mut Task<T>) {
        let end = Instant::now() + Duration::from_secs(5);
        while task.is_running() && Instant::now() < end {
            task.poll();
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn task_runs_to_done_with_progress() {
        let rt = runner();
        let mut task: Task<u32> = Task::default();
        assert_eq!(task.status, Status::Idle);
        task.start(rt.run(|tx| {
            tx.send(Msg::Progress {
                done: 1,
                total: 2,
                note: "half".into(),
            });
            Ok(7)
        }));
        assert!(task.is_running());
        wait(&mut task);
        assert_eq!(task.status, Status::Done(7));
    }

    #[test]
    fn task_reports_failure() {
        let rt = runner();
        let mut task: Task<u32> = Task::default();
        task.start(rt.run(|_| Err(avlt_core::Error::Other("boom".into()))));
        wait(&mut task);
        assert_eq!(task.status, Status::Failed("boom".into()));
    }

    #[test]
    fn cancel_returns_to_idle_and_stops_the_job() {
        let rt = runner();
        let mut task: Task<u32> = Task::default();
        let started = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let (s1, s2) = (Arc::clone(&started), Arc::clone(&stopped));
        task.start(rt.run(move |tx| {
            s1.store(true, Ordering::Relaxed);
            while !tx.cancelled() {
                std::thread::sleep(Duration::from_millis(2));
            }
            s2.store(true, Ordering::Relaxed);
            Ok(1)
        }));
        let end = Instant::now() + Duration::from_secs(5);
        while !started.load(Ordering::Relaxed) && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(2));
        }
        task.cancel();
        assert_eq!(task.status, Status::Idle);
        let end = Instant::now() + Duration::from_secs(5);
        while !stopped.load(Ordering::Relaxed) && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(stopped.load(Ordering::Relaxed), "job saw the cancel flag");
        task.poll();
        assert_eq!(task.status, Status::Idle, "late result is ignored");
    }

    #[test]
    fn dropping_the_runner_cancels_running_jobs() {
        let rt = runner();
        let saw_cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&saw_cancel);
        let started = Arc::new(AtomicBool::new(false));
        let s = Arc::clone(&started);
        let job = rt.spawn_blocking(move |tx: Sender<()>| {
            s.store(true, Ordering::Relaxed);
            while !tx.cancelled() {
                std::thread::sleep(Duration::from_millis(2));
            }
            flag.store(true, Ordering::Relaxed);
        });
        while !started.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(2));
        }
        let t = Instant::now();
        drop(rt);
        assert!(t.elapsed() < Duration::from_secs(2), "exit wait is bounded");
        assert!(saw_cancel.load(Ordering::Relaxed), "the job saw the cancel");
        drop(job);
    }

    #[test]
    fn messages_outside_running_are_ignored() {
        let mut task: Task<u32> = Task::default();
        task.update(Msg::Finished(Ok(3)));
        assert_eq!(task.status, Status::Idle);
    }

    #[test]
    fn live_keeps_a_bounded_history_and_stops_on_failure() {
        let rt = runner();
        let mut live: Live<usize> = Live::default();
        live.start(rt.spawn(|tx| async move {
            for i in 0..(HISTORY + 5) {
                tx.send(LiveMsg::Sample(i));
            }
            tx.send(LiveMsg::Failed("gone".into()));
        }));
        let end = Instant::now() + Duration::from_secs(5);
        while live.is_running() && Instant::now() < end {
            live.poll();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!live.is_running());
        assert_eq!(live.samples.len(), HISTORY);
        assert_eq!(live.last(), Some(&(HISTORY + 4)));
        assert_eq!(live.error.as_deref(), Some("gone"));
    }

    #[test]
    fn live_stop_keeps_samples() {
        let rt = runner();
        let mut live: Live<u8> = Live::default();
        live.start(rt.spawn(|_tx| std::future::pending::<()>()));
        live.update(LiveMsg::Sample(1));
        live.stop();
        assert!(!live.is_running());
        live.update(LiveMsg::Sample(2));
        assert_eq!(live.samples, [1]);
    }
}
