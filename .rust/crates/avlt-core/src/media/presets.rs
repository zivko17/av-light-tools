//! Audio presets, channel layouts and extraction formats (port of `$script:AudioCodecMeta`
//! and the format switches of `AudioTools.ps1`).

use std::str::FromStr;

use crate::Error;

/// Output preset of `avlt media convert`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Mp3,
    Mp3Low,
    Aac,
    Opus,
    Flac,
    Wav16,
    Wav24,
}

impl Preset {
    /// Every preset, in menu order.
    pub const ALL: [Preset; 7] = [
        Preset::Mp3,
        Preset::Mp3Low,
        Preset::Aac,
        Preset::Opus,
        Preset::Flac,
        Preset::Wav16,
        Preset::Wav24,
    ];

    /// Name used on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Preset::Mp3 => "mp3",
            Preset::Mp3Low => "mp3-low",
            Preset::Aac => "aac",
            Preset::Opus => "opus",
            Preset::Flac => "flac",
            Preset::Wav16 => "wav16",
            Preset::Wav24 => "wav24",
        }
    }

    /// Label used in output file names (`name_MP3-320.mp3`).
    pub fn label(self) -> &'static str {
        match self {
            Preset::Mp3 => "MP3-320",
            Preset::Mp3Low => "MP3-192",
            Preset::Aac => "AAC-256",
            Preset::Opus => "Opus-192",
            Preset::Flac => "FLAC",
            Preset::Wav16 => "WAV-16",
            Preset::Wav24 => "WAV-24",
        }
    }

    /// Output extension with the dot.
    pub fn ext(self) -> &'static str {
        match self {
            Preset::Mp3 | Preset::Mp3Low => ".mp3",
            Preset::Aac => ".m4a",
            Preset::Opus => ".opus",
            Preset::Flac => ".flac",
            Preset::Wav16 | Preset::Wav24 => ".wav",
        }
    }

    /// One-line description for help texts.
    pub fn description(self) -> &'static str {
        match self {
            Preset::Mp3 => "MP3 320 kbps (high quality, universal)",
            Preset::Mp3Low => "MP3 192 kbps (standard)",
            Preset::Aac => "AAC 256 kbps (better than MP3)",
            Preset::Opus => "Opus 192 kbps (best quality, open)",
            Preset::Flac => "FLAC (lossless, compressed)",
            Preset::Wav16 => "WAV 16-bit (CD quality)",
            Preset::Wav24 => "WAV 24-bit (master)",
        }
    }

    /// The FFmpeg codec arguments of the preset.
    pub fn codec_args(self) -> &'static [&'static str] {
        match self {
            Preset::Mp3 => &["-c:a", "libmp3lame", "-b:a", "320k"],
            Preset::Mp3Low => &["-c:a", "libmp3lame", "-b:a", "192k"],
            Preset::Aac => &["-c:a", "aac", "-b:a", "256k"],
            Preset::Opus => &["-c:a", "libopus", "-b:a", "192k"],
            Preset::Flac => &["-c:a", "flac", "-compression_level", "5"],
            Preset::Wav16 => &["-c:a", "pcm_s16le"],
            Preset::Wav24 => &["-c:a", "pcm_s24le"],
        }
    }
}

impl FromStr for Preset {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        let key = s.trim().to_ascii_lowercase();
        Preset::ALL
            .into_iter()
            .find(|p| p.name() == key)
            .ok_or_else(|| {
                let all: Vec<&str> = Preset::ALL.iter().map(|p| p.name()).collect();
                Error::InvalidInput(format!(
                    "unknown preset '{s}' (expected one of: {})",
                    all.join(", ")
                ))
            })
    }
}

/// Channel layout of `avlt media channels` / `--channels`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channels {
    Mono,
    Stereo,
}

impl Channels {
    /// Name used on the command line and in output file names.
    pub fn name(self) -> &'static str {
        match self {
            Channels::Mono => "mono",
            Channels::Stereo => "stereo",
        }
    }

    /// Number of channels (`-ac`).
    pub fn count(self) -> u8 {
        match self {
            Channels::Mono => 1,
            Channels::Stereo => 2,
        }
    }
}

impl FromStr for Channels {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mono" => Ok(Channels::Mono),
            "stereo" => Ok(Channels::Stereo),
            _ => Err(Error::InvalidInput(format!(
                "unknown channel layout '{s}' (expected mono or stereo)"
            ))),
        }
    }
}

/// Output of `avlt media extract`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractFormat {
    Mp3,
    Flac,
    Wav,
    /// Stream copy into an `.m4a` container.
    Copy,
}

impl ExtractFormat {
    /// Name used on the command line.
    pub fn name(self) -> &'static str {
        match self {
            ExtractFormat::Mp3 => "mp3",
            ExtractFormat::Flac => "flac",
            ExtractFormat::Wav => "wav",
            ExtractFormat::Copy => "copy",
        }
    }

    /// Output extension with the dot.
    pub fn ext(self) -> &'static str {
        match self {
            ExtractFormat::Mp3 => ".mp3",
            ExtractFormat::Flac => ".flac",
            ExtractFormat::Wav => ".wav",
            ExtractFormat::Copy => ".m4a",
        }
    }
}

impl FromStr for ExtractFormat {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mp3" => Ok(ExtractFormat::Mp3),
            "flac" => Ok(ExtractFormat::Flac),
            "wav" => Ok(ExtractFormat::Wav),
            "copy" => Ok(ExtractFormat::Copy),
            _ => Err(Error::InvalidInput(format!(
                "unknown extract format '{s}' (expected mp3, flac, wav or copy)"
            ))),
        }
    }
}

/// Sample rates accepted by `avlt media resample`.
pub const SAMPLE_RATES: [u32; 4] = [44_100, 48_000, 96_000, 192_000];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_names_round_trip() {
        for p in Preset::ALL {
            assert_eq!(p.name().parse::<Preset>().unwrap(), p);
        }
        assert_eq!("MP3-LOW".parse::<Preset>().unwrap(), Preset::Mp3Low);
        let err = "ogg".parse::<Preset>().unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("mp3-low"), "{err}");
    }

    #[test]
    fn preset_table_matches_audiotools_ps1() {
        let table: Vec<(&str, &str, String)> = Preset::ALL
            .iter()
            .map(|p| (p.label(), p.ext(), p.codec_args().join(" ")))
            .collect();
        assert_eq!(
            table,
            [
                ("MP3-320", ".mp3", "-c:a libmp3lame -b:a 320k".to_string()),
                ("MP3-192", ".mp3", "-c:a libmp3lame -b:a 192k".to_string()),
                ("AAC-256", ".m4a", "-c:a aac -b:a 256k".to_string()),
                ("Opus-192", ".opus", "-c:a libopus -b:a 192k".to_string()),
                (
                    "FLAC",
                    ".flac",
                    "-c:a flac -compression_level 5".to_string()
                ),
                ("WAV-16", ".wav", "-c:a pcm_s16le".to_string()),
                ("WAV-24", ".wav", "-c:a pcm_s24le".to_string()),
            ]
        );
    }

    #[test]
    fn channels_and_formats_parse() {
        assert_eq!("Mono".parse::<Channels>().unwrap().count(), 1);
        assert_eq!("stereo".parse::<Channels>().unwrap().count(), 2);
        assert!("5.1".parse::<Channels>().is_err());
        assert_eq!("copy".parse::<ExtractFormat>().unwrap().ext(), ".m4a");
        assert!("aac".parse::<ExtractFormat>().is_err());
    }
}
