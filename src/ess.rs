use crate::traits::HandleFrame;

#[derive(Debug, Clone, Default)]
pub struct Ess {
    /// Setpoint relative to the grid: positive imports, negative exports
    /// The read setpoint can be different from the one that is set, as the
    /// multiplus ramps ups the setpoint gradually
    /// Warning: only supports single-phase (L1) for now
    pub grid_setpoint: Option<f64>,
}

impl HandleFrame for Ess {
    fn handle_frame(&mut self, parts: &[&str], value: Option<f64>) {
        match parts {
            ["L1", "AcPowerSetpoint"] => self.grid_setpoint = value,
            _ => {
                tracing::warn!("Unhandled Ess parts: {:?}, value: {:?}", parts, value);
            }
        }
    }
}
