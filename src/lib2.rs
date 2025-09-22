use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{Notify, RwLock};
use tokio::task;
use tokio::time::{self, Duration};
use tracing::{error, info};

#[derive(Debug, Clone, Default)]
pub struct AcSpec {
    pub voltage: Option<f64>,
    pub power: Option<f64>,
    pub frequency: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct BatteryDC {
    pub temperature: Option<f64>,
    pub power: Option<f64>,
    pub current: Option<f64>,
    pub voltage: Option<f64>,
    pub soc: Option<f64>,
    pub soh: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct SolarPower {
    pub power: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct VictronData {
    pub ac_input: AcSpec,
    pub ac_output: AcSpec,
    pub battery_dc: BatteryDC,
    pub solar_power: SolarPower,
}

pub struct VictronGx {
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

        // Keepalive task
        let keepalive_topic = format!("R/{}/keepalive", serial_number);
        let client_clone = client.clone();
        let shutdown_clone = shutdown.clone();
        let _keepalive_handle = task::spawn(Self::keepalive_task(
            client_clone,
            keepalive_topic,
            shutdown_clone,
        ));

        // Subscribe to topics
        let topic_pattern = format!("N/{}/#", serial_number);
        client.subscribe(&topic_pattern, QoS::AtMostOnce).await?;

        // Event loop
        let data_clone = data.clone();
        let serial_number = serial_number.to_string();
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
                            Self::handle_publish(&data_clone, &serial_number, &publish.topic, &publish.payload).await;
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

        Ok(Self {
            data,
            client,
            shutdown,
            _eventloop_handle,
            _keepalive_handle,
        })
    }

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

        let topic_suffix = topic
            .strip_prefix(&format!("N/{}/", serial_number))
            .unwrap_or("");

        let mut data = data.write().await;
        match topic_suffix {
            "vebus/275/Ac/ActiveIn/L1/F" => data.ac_input.frequency = value,
            "vebus/275/Ac/ActiveIn/L1/P" => data.ac_input.power = value,
            "vebus/275/Ac/ActiveIn/L1/V" => data.ac_input.voltage = value,

            "vebus/275/Ac/Out/L1/F" => data.ac_output.frequency = value,
            "vebus/275/Ac/Out/L1/P" => data.ac_output.power = value,
            "vebus/275/Ac/Out/L1/V" => data.ac_output.voltage = value,

            "battery/512/Dc/0/Temperature" => data.battery_dc.temperature = value,
            "battery/512/Dc/0/Power" => data.battery_dc.power = value,
            "battery/512/Dc/0/Current" => data.battery_dc.current = value,
            "battery/512/Dc/0/Voltage" => data.battery_dc.voltage = value,
            "battery/512/Soc" => data.battery_dc.soc = value,
            "battery/512/Soh" => data.battery_dc.soh = value,

            "pvinverter/20/Power" => data.solar_power.power = value,
            _ => {}
        }
    }

    pub async fn get_ac_input(&self) -> AcSpec {
        self.data.read().await.ac_input.clone()
    }

    pub async fn get_ac_output(&self) -> AcSpec {
        self.data.read().await.ac_output.clone()
    }

    pub async fn get_battery_dc(&self) -> BatteryDC {
        self.data.read().await.battery_dc.clone()
    }

    pub async fn get_solar_power(&self) -> SolarPower {
        self.data.read().await.solar_power.clone()
    }

    pub async fn send_mqtt(&self, topic: &str, value: f64) -> anyhow::Result<()> {
        let payload = serde_json::json!({ "value": value }).to_string();
        self.client
            .publish(topic, QoS::AtLeastOnce, false, payload)
            .await?;
        Ok(())
    }

    pub async fn shutdown(&self) {
        self.shutdown.notify_waiters();
    }
}
