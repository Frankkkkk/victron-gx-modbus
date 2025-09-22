use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Notify, RwLock};
use tokio::task;
use tokio::time::{self, Duration};
use tracing::{debug, error, info};

use crate::ac::AcSpec;
use crate::battery::{BatteryDC, BatterySummary};
use crate::ess::Ess;
use crate::pvinverter::{PvInverter, PvInverterSummary};
use crate::traits::HandleFrame;

pub mod ac;
pub mod battery;
pub mod ess;
pub mod pvinverter;
pub mod traits;

#[derive(Debug, Clone, Default)]
pub struct VictronData {
    pub ac_input: AcSpec,
    pub ac_output: AcSpec,
    pub ess: Ess,
    pub batteries_dc: HashMap<u16, BatteryDC>,
    pub pv_inverters: HashMap<u16, PvInverter>,
}

pub struct VictronGx {
    pub serial_number: String,
    data: Arc<RwLock<VictronData>>,
    client: AsyncClient,

    shutdown: Arc<Notify>,

    _eventloop_handle: task::JoinHandle<()>,
    _keepalive_handle: task::JoinHandle<()>,
}

impl VictronGx {
    pub async fn new(
        client_id: &str,
        host: &str,
        port: u16,
        serial_number: &str,
    ) -> anyhow::Result<Self> {
        let mut mqttoptions = MqttOptions::new(client_id, host, port);
        mqttoptions.set_keep_alive(Duration::from_secs(30));

        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
        let data = Arc::new(RwLock::new(VictronData::default()));
        let shutdown = Arc::new(Notify::new());

        let keepalive_topic = format!("R/{}/keepalive", serial_number);
        let client_clone = client.clone();
        let shutdown_clone = shutdown.clone();

        let _keepalive_handle = task::spawn(Self::keepalive_task(
            client_clone,
            keepalive_topic,
            shutdown_clone,
        ));

        // Subscribe to topics with the serial number
        let topics = vec![format!("N/{}/#", serial_number)];

        for topic in &topics {
            client.subscribe(topic, QoS::AtMostOnce).await?;
        }

        let data_clone = data.clone();
        let serial_number_cpy = serial_number.to_string();

        let shutdown_clone = shutdown.clone();
        let _eventloop_handle = task::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_clone.notified() => {
                        info!("Shutting down event loop");
                        break;
                    }
                    event = eventloop.poll() => match event {
                        Ok(Event::Incoming(Packet::Publish(publish))) => {
                            Self::handle_publish(&data_clone, &serial_number_cpy, &publish.topic, &publish.payload).await;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            error!("MQTT event loop error: {:?}", e);
                            time::sleep(Duration::from_secs(5)).await;
                        }
                    }
                }
            }
        });

        // Schedules
        /*
        N/028102353a50/settings/0/Settings/CGwacs/BatteryLife/Schedule/Charge/0/AllowDischarge -> b'{"value": 0}'
        N/028102353a50/settings/0/Settings/CGwacs/BatteryLife/Schedule/Charge/0/Day -> b'{"value": 7}'
        N/028102353a50/settings/0/Settings/CGwacs/BatteryLife/Schedule/Charge/0/Duration -> b'{"value": 13800}'
        N/028102353a50/settings/0/Settings/CGwacs/BatteryLife/Schedule/Charge/0/Soc -> b'{"value": 55}'
        N/028102353a50/settings/0/Settings/CGwacs/BatteryLife/Schedule/Charge/0/Start -> b'{"value": 7200}'
        */

        Ok(Self {
            serial_number: serial_number.to_string(),
            data,
            client,
            shutdown,
            _eventloop_handle,
            _keepalive_handle,
        })
    }

    async fn handle_publish(
        data: &Arc<RwLock<VictronData>>,
        serial_number: &str,
        topic: &str,
        payload: &[u8],
    ) {
        let payload_str = match std::str::from_utf8(payload) {
            Ok(s) => s,
            Err(e) => {
                error!("Invalid UTF-8 payload: {:?}", e);
                return;
            }
        };

        let value = match serde_json::from_str::<Value>(payload_str) {
            Ok(json) => json.get("value").and_then(|v| v.as_f64()),
            Err(e) => {
                error!("JSON parse error: {:?}", e);
                None
            }
        };

        let suffix = match topic.strip_prefix(&format!("N/{}/", serial_number)) {
            Some(s) => s,
            None => return,
        };

        let parts: Vec<&str> = suffix.split('/').collect();
        let mut data = data.write().await;

        match parts.as_slice() {
            ["vebus", "275", "Ac", "ActiveIn", rest @ ..] => {
                data.ac_input.handle_frame(rest, value)
            }
            ["vebus", "275", "Ac", "Out", rest @ ..] => data.ac_output.handle_frame(rest, value),
            ["battery", id, rest @ ..] => {
                let id: u16 = id.parse().unwrap_or(0);
                let battery = data.batteries_dc.entry(id).or_default();
                battery.handle_frame(rest, value);
            }
            ["pvinverter", id, rest @ ..] => {
                let id: u16 = id.parse().unwrap_or(0);
                let inverter = data.pv_inverters.entry(id).or_default();
                inverter.handle_frame(rest, value);
            }
            ["vebus", "275", "Hub4", rest @ ..] => {
                data.ess.handle_frame(rest, value);
            }
            _ => {
                debug!(
                    "Unhandled topic: {}, parts: {:?}, value: {:?}",
                    topic, parts, value
                );
            }
        }
    }
    /// The MQTT server "shuts-down" (basically stops publishing anything) if
    /// no messages are received by it for a minute or so
    async fn keepalive_task(client: AsyncClient, topic: String, shutdown: Arc<Notify>) {
        let mut interval = time::interval(Duration::from_secs(10));
        loop {
            tokio::select! {
                _ = shutdown.notified() => {
                    info!("Shutting down keepalive task");
                    break;
                }
                _ = interval.tick() => {
                    if let Err(e) = client.publish(&topic, QoS::AtLeastOnce, false, "").await {
                        error!("Keepalive publish failed: {:?}", e);
                    }
                }
            }
        }
    }

    pub async fn shutdown(&self) {
        self.shutdown.notify_waiters();
    }

    pub async fn get_ac_input(&self) -> AcSpec {
        self.data.read().await.ac_input.clone()
    }
    pub async fn get_ac_output(&self) -> AcSpec {
        self.data.read().await.ac_output.clone()
    }

    pub async fn get_batteries_dc(&self) -> Vec<(u16, BatteryDC)> {
        self.data
            .read()
            .await
            .batteries_dc
            .iter()
            .map(|(id, batt)| (*id, batt.clone()))
            .collect()
    }

    /// Returns a summary of all batteries (average voltage, average temperature, total power)
    pub async fn get_battery_summary(&self) -> BatterySummary {
        let data = self.data.read().await;
        BatterySummary::from_batteries(data.batteries_dc.values())
    }

    pub async fn get_pv_inverters(&self) -> Vec<(u16, PvInverter)> {
        self.data
            .read()
            .await
            .pv_inverters
            .iter()
            .map(|(id, inv)| (*id, inv.clone()))
            .collect()
    }

    /// Returns a summary of all pv inverters (total power)
    pub async fn get_pv_inverter_summary(&self) -> PvInverterSummary {
        let data = self.data.read().await;
        PvInverterSummary::from_pv_inverters(data.pv_inverters.values())
    }

    // TODO FIXME
    pub async fn set_ac_power_setpoint(&self) {
        // XXX this just sets the setpoint. But the power going into the batteries
        // depends on the consumption of the house
        let topic = "W/028102353a50/settings/0/Settings/CGwacs/AcPowerSetPoint";
        let content = r#"{"value": 1234}"#;
        if let Err(e) = self
            .client
            .publish(topic, QoS::AtLeastOnce, false, content)
            .await
        {
            error!("Failed to set AC power setpoint: {:?}", e);
        }
    }

    pub async fn get_ess(&self) -> Ess {
        self.data.read().await.ess.clone()
    }

    pub async fn ess_set_setpoint(&self, value: f64) -> anyhow::Result<()> {
        let topic = format!(
            "W/{}/settings/0/Settings/CGwacs/AcPowerSetPoint",
            self.serial_number
        );
        let content = serde_json::json!({ "value": value }).to_string();

        self.client
            .publish(topic, QoS::AtLeastOnce, false, content)
            .await?;

        Ok(())
    }

    pub async fn send_mqtt(&self) -> anyhow::Result<()> {
        let topic = "W/028102353a50/vebus/275/Mode";
        let topic = "W/028102353a50/vebus/275/Hub4/L1/AcPowerSetpoint";
        let topic = "W/028102353a50/settings/0/Settings/CGwacs/AcPowerSetPoint";
        let content = r#"{"value": 0}"#;

        self.client
            .publish(topic, QoS::AtLeastOnce, false, content)
            .await?;

        Ok(())
    }
}
