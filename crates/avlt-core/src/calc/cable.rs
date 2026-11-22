//! Speaker wire loss and coax (SDI / RF) attenuation.
//! Ports `Get-CableSize` (AVCalc.ps1 lines 936-965).

use serde::Serialize;

use super::{require_non_negative, require_positive};
use crate::error::{Error, Result};

/// Copper resistance per metre of conductor by AWG (AVCalc.ps1 line 946).
pub const AWG_OHM_PER_M: [(u32, f64); 6] = [
    (10, 0.00328),
    (12, 0.00521),
    (14, 0.00829),
    (16, 0.01317),
    (18, 0.02095),
    (20, 0.03331),
];

/// Cable type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CableType {
    /// Speaker wire: resistive loss into a load.
    #[default]
    Speaker,
    /// Coax: attenuation in dB per 100 m.
    Coax,
}

/// Inputs.
#[derive(Debug, Clone, PartialEq)]
pub struct CableInput {
    pub kind: CableType,
    /// Wire gauge (speaker): 10, 12, 14, 16, 18 or 20.
    pub awg: u32,
    /// Cable length (m).
    pub length_m: f64,
    /// Speaker load (ohms).
    pub impedance: f64,
    /// Coax attenuation (dB per 100 m at the working frequency).
    pub atten_per_100m: f64,
    /// Coax attenuation budget (dB) -> maximum length; 0 = skip.
    pub max_db: f64,
}

impl Default for CableInput {
    fn default() -> Self {
        CableInput {
            kind: CableType::Speaker,
            awg: 14,
            length_m: 0.0,
            impedance: 8.0,
            atten_per_100m: 0.0,
            max_db: 0.0,
        }
    }
}

/// Quality of a speaker run by power loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CableRating {
    /// 0.5 dB or less.
    Good,
    /// Up to 1 dB.
    Marginal,
    /// Above 1 dB: go thicker.
    Poor,
}

/// Cable calculation; fields not relevant to the cable type are `None`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CableResult {
    pub kind: CableType,
    pub awg: Option<u32>,
    pub length_m: Option<f64>,
    pub impedance: Option<f64>,
    /// Round-trip cable resistance.
    pub resistance_ohm: Option<f64>,
    pub loss_db: Option<f64>,
    pub loss_percent: Option<f64>,
    pub rating: Option<CableRating>,
    pub atten_per_100m: Option<f64>,
    /// Attenuation over `length_m` (coax).
    pub attenuation_db: Option<f64>,
    /// Longest run within `max_db` (coax).
    pub max_length_m: Option<f64>,
}

/// Speaker cable power loss, or coax attenuation and maximum length.
pub fn cable(input: &CableInput) -> Result<CableResult> {
    let length = require_non_negative("length-m", input.length_m)?;
    let none = CableResult {
        kind: input.kind,
        awg: None,
        length_m: None,
        impedance: None,
        resistance_ohm: None,
        loss_db: None,
        loss_percent: None,
        rating: None,
        atten_per_100m: None,
        attenuation_db: None,
        max_length_m: None,
    };

    match input.kind {
        CableType::Speaker => {
            let ohm_per_m = AWG_OHM_PER_M
                .iter()
                .find(|(awg, _)| *awg == input.awg)
                .map(|(_, r)| *r)
                .ok_or_else(|| {
                    let list: Vec<String> =
                        AWG_OHM_PER_M.iter().map(|(a, _)| a.to_string()).collect();
                    Error::InvalidInput(format!("AWG must be one of: {}", list.join(", ")))
                })?;
            if length <= 0.0 {
                return Err(Error::InvalidInput(
                    "give a --length-m (e.g. calc cable --awg 14 --length-m 30)".into(),
                ));
            }
            let impedance = require_positive("impedance", input.impedance)?;
            let rc = ohm_per_m * 2.0 * length; // round trip
            let loss_db = 10.0 * ((impedance + rc) / impedance).log10();
            let rating = if loss_db <= 0.5 {
                CableRating::Good
            } else if loss_db <= 1.0 {
                CableRating::Marginal
            } else {
                CableRating::Poor
            };
            Ok(CableResult {
                awg: Some(input.awg),
                length_m: Some(length),
                impedance: Some(impedance),
                resistance_ohm: Some(rc),
                loss_db: Some(loss_db),
                loss_percent: Some(rc / (rc + impedance) * 100.0),
                rating: Some(rating),
                ..none
            })
        }
        CableType::Coax => {
            let atten = require_positive("atten-per-100m", input.atten_per_100m)?;
            let max_db = require_non_negative("max-db", input.max_db)?;
            if length <= 0.0 && max_db <= 0.0 {
                return Err(Error::InvalidInput(
                    "give a --length-m and/or a --max-db budget".into(),
                ));
            }
            Ok(CableResult {
                atten_per_100m: Some(atten),
                length_m: (length > 0.0).then_some(length),
                attenuation_db: (length > 0.0).then(|| atten * length / 100.0),
                max_length_m: (max_db > 0.0).then(|| max_db * 100.0 / atten),
                ..none
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    fn speaker(awg: u32, length: f64, ohms: f64) -> CableInput {
        CableInput {
            awg,
            length_m: length,
            impedance: ohms,
            ..CableInput::default()
        }
    }

    fn coax(atten: f64) -> CableInput {
        CableInput {
            kind: CableType::Coax,
            atten_per_100m: atten,
            ..CableInput::default()
        }
    }

    #[test]
    fn speaker_14awg_30m() {
        // lines 951-956: rc = 0.00829 * 2 * 30 = 0.4974 ohm; loss = 10*log10((8 + rc)/8) = 0.26196 dB;
        // percent = rc / (rc + 8) * 100 = 5.8536 %; <= 0.5 dB -> good
        let r = cable(&speaker(14, 30.0, 8.0)).unwrap();
        close(r.resistance_ohm.unwrap(), 0.4974);
        close(r.loss_db.unwrap(), 0.261_960_753_83);
        close(r.loss_percent.unwrap(), 5.853_555_205_12);
        assert_eq!(r.rating, Some(CableRating::Good));
        assert_eq!(r.awg, Some(14));
        assert!(r.attenuation_db.is_none() && r.max_length_m.is_none());
    }

    #[test]
    fn speaker_marginal_and_poor() {
        // 16 AWG 50 m 8 ohm: rc = 1.317; loss = 0.66186 dB -> marginal (0.5 < x <= 1)
        let r = cable(&speaker(16, 50.0, 8.0)).unwrap();
        close(r.loss_db.unwrap(), 0.661_861_084_97);
        assert_eq!(r.rating, Some(CableRating::Marginal));
        // 18 AWG 100 m 4 ohm: rc = 4.19; loss = 3.11224 dB -> poor
        let r = cable(&speaker(18, 100.0, 4.0)).unwrap();
        close(r.resistance_ohm.unwrap(), 4.19);
        close(r.loss_db.unwrap(), 3.112_239_104_32);
        assert_eq!(r.rating, Some(CableRating::Poor));
    }

    #[test]
    fn every_listed_awg_is_accepted() {
        for (awg, _) in AWG_OHM_PER_M {
            assert!(cable(&speaker(awg, 10.0, 8.0)).is_ok(), "awg {awg}");
        }
    }

    #[test]
    fn coax_attenuation_and_max_length() {
        // lines 960-961: 20 dB/100 m over 80 m = 16 dB; budget 20 dB -> 20 * 100 / 20 = 100 m
        let r = cable(&CableInput {
            length_m: 80.0,
            max_db: 20.0,
            ..coax(20.0)
        })
        .unwrap();
        close(r.attenuation_db.unwrap(), 16.0);
        close(r.max_length_m.unwrap(), 100.0);
        assert!(r.loss_db.is_none() && r.rating.is_none());
        // length only
        let r = cable(&CableInput {
            length_m: 50.0,
            ..coax(20.0)
        })
        .unwrap();
        close(r.attenuation_db.unwrap(), 10.0);
        assert!(r.max_length_m.is_none());
    }

    #[test]
    fn invalid_input() {
        for bad in [
            // AWG not in the table
            speaker(13, 30.0, 8.0),
            speaker(14, 0.0, 8.0),
            speaker(14, -30.0, 8.0),
            speaker(14, 30.0, 0.0),
            speaker(14, 30.0, -8.0),
            speaker(14, f64::NAN, 8.0),
            // coax: no attenuation, or nothing to compute
            coax(0.0),
            coax(-5.0),
            coax(20.0),
            CableInput {
                length_m: -1.0,
                ..coax(20.0)
            },
            CableInput {
                max_db: -1.0,
                length_m: 10.0,
                ..coax(20.0)
            },
        ] {
            assert!(
                matches!(cable(&bad), Err(Error::InvalidInput(_))),
                "{bad:?}"
            );
        }
    }
}
