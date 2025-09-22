use tracing::debug;

use crate::traits::HandleFrame;

#[derive(Debug, Clone, Default)]
pub struct PvInverter {
    pub power: Option<f64>,
    pub max_power: Option<f64>,

    pub total_energy: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct PvInverterSummary {
    pub total_power: Option<f64>,
}

impl HandleFrame for PvInverter {
    fn handle_frame(&mut self, parts: &[&str], value: Option<f64>) {
        match parts {
            ["Ac", "Power"] => self.power = value,
            ["Ac", "MaxPower"] => self.max_power = value,
            ["Ac", "Energy", "Forward"] => self.total_energy = value,
            _ => {
                debug!(
                    "Unhandled PvInverter parts: {:?}, value: {:?}",
                    parts, value
                );
            }
        }
    }
}

impl PvInverterSummary {
    pub fn from_pv_inverters<'a, I>(inverters: I) -> Self
    where
        I: Iterator<Item = &'a PvInverter> + Clone,
    {
        Self {
            total_power: {
                let values: Vec<f64> = inverters.clone().filter_map(|b| b.power).collect();
                if values.is_empty() {
                    None
                } else {
                    Some(values.into_iter().sum())
                }
            },
        }
    }
}
