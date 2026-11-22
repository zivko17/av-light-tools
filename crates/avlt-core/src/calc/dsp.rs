//! DSP helpers: dBu conversion, latency budget and matrix size.
//! Ports `Get-DSPSystem` (AVCalc.ps1 lines 969-993).

use serde::Serialize;

use crate::error::{Error, Result};

/// 0 dBu = 0.7746 Vrms.
pub const DBU_REFERENCE_VRMS: f64 = 0.7746;

/// Inputs; each part is optional, at least one is required.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DspInput {
    /// Level to convert (dBu).
    pub dbu: Option<f64>,
    /// Latency chain to sum (ms).
    pub latencies_ms: Vec<f64>,
    /// Matrix inputs (with `outputs`).
    pub inputs: u32,
    pub outputs: u32,
}

/// dBu converted to volts and dBV.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DbuConversion {
    pub dbu: f64,
    pub vrms: f64,
    pub dbv: f64,
}

/// Summed latency chain.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LatencyChain {
    pub chain_ms: Vec<f64>,
    pub total_ms: f64,
}

/// Matrix size.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MatrixSize {
    pub inputs: u32,
    pub outputs: u32,
    pub crosspoints: u64,
}

/// Whichever parts were requested.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DspResult {
    pub level: Option<DbuConversion>,
    pub latency: Option<LatencyChain>,
    pub matrix: Option<MatrixSize>,
}

/// Gain conversion, latency budget and matrix crosspoints.
pub fn dsp(input: &DspInput) -> Result<DspResult> {
    if let Some(dbu) = input.dbu
        && !dbu.is_finite()
    {
        return Err(Error::InvalidInput(format!(
            "dBu must be a finite number (got {dbu})"
        )));
    }
    if let Some(bad) = input
        .latencies_ms
        .iter()
        .find(|v| !v.is_finite() || **v < 0.0)
    {
        return Err(Error::InvalidInput(format!(
            "latencies must be 0 or greater (got {bad})"
        )));
    }

    let level = input.dbu.map(|dbu| {
        let vrms = DBU_REFERENCE_VRMS * 10f64.powf(dbu / 20.0);
        DbuConversion {
            dbu,
            vrms,
            dbv: 20.0 * vrms.log10(),
        }
    });
    let latency = (!input.latencies_ms.is_empty()).then(|| LatencyChain {
        chain_ms: input.latencies_ms.clone(),
        total_ms: input.latencies_ms.iter().sum(),
    });
    let matrix = (input.inputs > 0 && input.outputs > 0).then(|| MatrixSize {
        inputs: input.inputs,
        outputs: input.outputs,
        crosspoints: u64::from(input.inputs) * u64::from(input.outputs),
    });

    if level.is_none() && latency.is_none() && matrix.is_none() {
        return Err(Error::InvalidInput(
            "give --dbu, --latencies-ms, or both --inputs and --outputs".into(),
        ));
    }
    Ok(DspResult {
        level,
        latency,
        matrix,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn dbu_conversion() {
        // line 979-980: 4 dBu -> 0.7746 * 10^(4/20) = 1.227658 Vrms; dBV = 20*log10(vrms) = 1.78155
        let r = dsp(&DspInput {
            dbu: Some(4.0),
            ..DspInput::default()
        })
        .unwrap();
        let l = r.level.unwrap();
        close(l.dbu, 4.0);
        close(l.vrms, 1.227_658_266_880);
        close(l.dbv, 1.781_549_853_001);
        assert!(r.latency.is_none() && r.matrix.is_none());
    }

    #[test]
    fn zero_dbu_is_the_reference_voltage() {
        // 0 dBu = 0.7746 Vrms = -2.21845 dBV
        let l = dsp(&DspInput {
            dbu: Some(0.0),
            ..DspInput::default()
        })
        .unwrap()
        .level
        .unwrap();
        close(l.vrms, 0.7746);
        close(l.dbv, -2.218_450_146_999);
    }

    #[test]
    fn negative_dbu_is_valid() {
        let l = dsp(&DspInput {
            dbu: Some(-10.0),
            ..DspInput::default()
        })
        .unwrap()
        .level
        .unwrap();
        close(l.vrms, 0.7746 * 10f64.powf(-0.5));
    }

    #[test]
    fn latency_chain_sum() {
        // line 983: 0.25 + 1.0 + 2.0 = 3.25 ms
        let r = dsp(&DspInput {
            latencies_ms: vec![0.25, 1.0, 2.0],
            ..DspInput::default()
        })
        .unwrap();
        let c = r.latency.unwrap();
        close(c.total_ms, 3.25);
        assert_eq!(c.chain_ms, vec![0.25, 1.0, 2.0]);
    }

    #[test]
    fn matrix_crosspoints() {
        // line 986: 16 x 8 = 128
        let m = dsp(&DspInput {
            inputs: 16,
            outputs: 8,
            ..DspInput::default()
        })
        .unwrap()
        .matrix
        .unwrap();
        assert_eq!(m.crosspoints, 128);
        // large matrices do not overflow
        let m = dsp(&DspInput {
            inputs: u32::MAX,
            outputs: u32::MAX,
            ..DspInput::default()
        })
        .unwrap()
        .matrix
        .unwrap();
        assert_eq!(m.crosspoints, u64::from(u32::MAX) * u64::from(u32::MAX));
    }

    #[test]
    fn all_parts_together() {
        let r = dsp(&DspInput {
            dbu: Some(4.0),
            latencies_ms: vec![1.0],
            inputs: 2,
            outputs: 2,
        })
        .unwrap();
        assert!(r.level.is_some() && r.latency.is_some() && r.matrix.is_some());
    }

    #[test]
    fn invalid_input() {
        for bad in [
            // nothing requested
            DspInput::default(),
            // a matrix needs both sides
            DspInput {
                inputs: 16,
                ..DspInput::default()
            },
            DspInput {
                outputs: 8,
                ..DspInput::default()
            },
            DspInput {
                dbu: Some(f64::NAN),
                ..DspInput::default()
            },
            DspInput {
                dbu: Some(f64::INFINITY),
                ..DspInput::default()
            },
            DspInput {
                latencies_ms: vec![1.0, -0.5],
                ..DspInput::default()
            },
            DspInput {
                latencies_ms: vec![f64::NAN],
                ..DspInput::default()
            },
        ] {
            assert!(matches!(dsp(&bad), Err(Error::InvalidInput(_))), "{bad:?}");
        }
    }
}
