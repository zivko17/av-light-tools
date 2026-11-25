//! Shared helpers for subcommands that run async work or live monitors: the Tokio
//! runtime, the sample pacer (Ctrl+C / `-n`) and ANSI colouring of table rows.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use avlt_core::Result;
use avlt_core::sys::format::Level;
use tokio::time::MissedTickBehavior;

use super::output::Out;

/// Runs `fut` on a fresh multi-threaded Tokio runtime. Blocking lookups still running
/// at the end (slow reverse DNS) are abandoned instead of delaying exit.
pub(crate) fn block_on<F: Future>(fut: F) -> Result<F::Output> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let value = rt.block_on(fut);
    rt.shutdown_background();
    Ok(value)
}

#[derive(Clone, Copy)]
pub(crate) enum Colour {
    Green,
    Yellow,
    Red,
    Cyan,
    Dim,
}

/// `s` in `colour` when stdout takes colour.
pub(crate) fn paint(out: &Out, colour: Colour, s: &str) -> String {
    if !out.color {
        return s.to_string();
    }
    let code = match colour {
        Colour::Green => "32",
        Colour::Yellow => "33",
        Colour::Red => "31",
        Colour::Cyan => "36",
        Colour::Dim => "2",
    };
    format!("\x1b[{code}m{s}\x1b[0m")
}

/// Green / yellow / red of a traffic-light level.
pub(crate) fn level_colour(level: Level) -> Colour {
    match level {
        Level::Good => Colour::Green,
        Level::Warn => Colour::Yellow,
        Level::Bad => Colour::Red,
    }
}

/// Paces a live monitor: one step per period until Ctrl+C or the sample limit.
pub(crate) struct Ticker {
    interval: tokio::time::Interval,
    stop: Pin<Box<dyn Future<Output = ()> + Send>>,
    remaining: Option<u64>,
}

impl Ticker {
    /// One step per second. `now`: first step immediately; otherwise after one period
    /// (rate monitors). Must be created inside the runtime.
    pub(crate) fn new(count: Option<u64>, now: bool) -> Ticker {
        Ticker::with_period(Duration::from_secs(1), count, now)
    }

    /// Like [`Ticker::new`] with a custom period.
    pub(crate) fn with_period(period: Duration, count: Option<u64>, now: bool) -> Ticker {
        let start = tokio::time::Instant::now() + if now { Duration::ZERO } else { period };
        let mut interval = tokio::time::interval_at(start, period);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        Ticker {
            interval,
            // If Ctrl+C cannot be hooked the monitor just runs until killed.
            stop: Box::pin(async {
                if tokio::signal::ctrl_c().await.is_err() {
                    std::future::pending::<()>().await;
                }
            }),
            remaining: count,
        }
    }

    /// Waits for the next step; `false` once the monitor must stop.
    pub(crate) async fn next(&mut self) -> bool {
        if self.remaining == Some(0) {
            return false;
        }
        tokio::select! {
            biased;
            () = &mut self.stop => return false,
            _ = self.interval.tick() => {}
        }
        if let Some(n) = &mut self.remaining {
            *n -= 1;
        }
        true
    }
}
