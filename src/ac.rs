use tracing::debug;

use crate::traits::HandleFrame;

#[derive(Debug, Clone, Default)]
/// AC specifications (input or output)
/// Voltage in V, Power in W, Frequency in Hz
/// WARNING: Only single-phase (L1) is currently supported
pub struct AcSpec {
    pub voltage: Option<f64>,
    pub power: Option<f64>,
    pub frequency: Option<f64>,
}

impl HandleFrame for AcSpec {
    fn handle_frame(&mut self, parts: &[&str], value: Option<f64>) {
        match parts {
            ["L1", "V"] => self.voltage = value,
            ["L1", "P"] => self.power = value,
            ["L1", "F"] => self.frequency = value,
            _ => {
                debug!("Unhandled AcSpec parts: {:?}, value: {:?}", parts, value);
            }
        }
    }
}
