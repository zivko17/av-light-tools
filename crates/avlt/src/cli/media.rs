//! `avlt media ...`: Media tools (ffmpeg / ffprobe).

use avlt_core::Result;
use clap::Subcommand;

use super::output::Out;
use super::{Pending, not_implemented};

/// `avlt media` subcommands.
#[derive(Debug, Subcommand)]
pub enum MediaCmd {
    /// Convert audio or video files
    Convert(Pending),
    /// Normalize audio loudness
    Normalize(Pending),
    /// Trim a media file
    Trim(Pending),
    /// Extract the audio track
    Extract(Pending),
    /// Change the audio sample rate
    Resample(Pending),
    /// Change the audio channel layout
    Channels(Pending),
    /// Show media file information
    Probe(Pending),
}

/// Runs an `avlt media` subcommand.
pub fn run(cmd: MediaCmd, out: &Out) -> Result<()> {
    let _ = out;
    match cmd {
        MediaCmd::Convert(_) => not_implemented(),
        MediaCmd::Normalize(_) => not_implemented(),
        MediaCmd::Trim(_) => not_implemented(),
        MediaCmd::Extract(_) => not_implemented(),
        MediaCmd::Resample(_) => not_implemented(),
        MediaCmd::Channels(_) => not_implemented(),
        MediaCmd::Probe(_) => not_implemented(),
    }
}
