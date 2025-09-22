use tracing::debug;

use crate::traits::HandleFrame;

#[derive(Debug, Clone, Default)]
pub struct BatteryDC {
    pub dc_power: Option<f64>,
    pub dc_current: Option<f64>,
    pub dc_voltage: Option<f64>,

    pub cell_min_voltage: Option<f64>,
    pub cell_max_voltage: Option<f64>,

    pub temperature: Option<f64>,
    pub soc: Option<f64>,
    pub soh: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct BatterySummary {
    // TODO: add rest of the fields
    pub avg_voltage: Option<f64>,
    pub avg_temperature: Option<f64>,
    pub total_power: Option<f64>,
}

impl HandleFrame for BatteryDC {
    fn handle_frame(&mut self, parts: &[&str], value: Option<f64>) {
        match parts {
            ["Dc", "0", "Temperature"] => self.temperature = value,
            ["Dc", "0", "Power"] => self.dc_power = value,
            ["Dc", "0", "Current"] => self.dc_current = value,
            ["Dc", "0", "Voltage"] => self.dc_voltage = value,

            ["System", "MinCellVoltage"] => self.cell_min_voltage = value,
            ["System", "MaxCellVoltage"] => self.cell_max_voltage = value,
            ["Soc"] => self.soc = value,
            ["Soh"] => self.soh = value,
            _ => {
                debug!("Unhandled BatteryDC parts: {:?}, value: {:?}", parts, value);
            }
        }
    }
}

impl BatterySummary {
    pub fn from_batteries<'a, I>(batteries: I) -> Self
    where
        I: Iterator<Item = &'a BatteryDC> + Clone,
    {
        Self {
            avg_voltage: None,     //TODO
            avg_temperature: None, // TODO
            total_power: {
                let values: Vec<f64> = batteries.clone().filter_map(|b| b.dc_power).collect();
                if values.is_empty() {
                    None
                } else {
                    Some(values.into_iter().sum())
                }
            },
        }
    }
}
