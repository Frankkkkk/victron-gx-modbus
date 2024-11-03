use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task;
use tokio::time::{self, Duration};

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
    // Keep the event loop handle so it isn't dropped
    _eventloop_handle: task::JoinHandle<()>,
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

        let keepalive_topic = format!("R/{}/keepalive", serial_number);
        let client2 = client.clone();
        // Publish keepalive message
        task::spawn(async move {
            loop {
                client2
                    .publish(&keepalive_topic, QoS::AtLeastOnce, false, "")
                    .await
                    .unwrap();

                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        });

        // Subscribe to topics with the serial number
        let topics = vec![format!("N/{}/#", serial_number)];

        for topic in &topics {
            client.subscribe(topic, QoS::AtMostOnce).await?;
        }

        let data = Arc::new(RwLock::new(VictronData::default()));
        let data_clone = data.clone();
        let serial_number = serial_number.to_string();

        // Move the event loop into the spawned task and keep the handle
        let _eventloop_handle = task::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(event) => {
                        if let Event::Incoming(Packet::Publish(publish)) = event {
                            let topic = publish.topic.clone();

                            let payload_bytes = &publish.payload;
                            let payload_str = String::from_utf8_lossy(payload_bytes);

                            // Parse the payload as JSON
                            let value: Option<f64> =
                                match serde_json::from_str::<serde_json::Value>(&payload_str) {
                                    Ok(json_value) => {
                                        json_value.get("value").and_then(|v| v.as_f64())
                                    }
                                    Err(_) => None,
                                };

                            //let payload = String::from_utf8_lossy(&publish.payload).to_string();
                            //let value: Option<f64> = payload.parse().ok();

                            let mut data = data_clone.write().await;

                            // Extract the part after the serial number
                            let topic_suffix = topic
                                .strip_prefix(&format!("N/{}/", serial_number))
                                .unwrap_or("");

                            println!(
                                "Topic Suffix: {}, Payload: {:?}\n",
                                topic_suffix, payload_bytes
                            );

                            match topic_suffix {
                                // XXX Topic Suffix: system/0/Ac/In/0/DeviceInstance, Payload: b"{\"value\": 275}"
                                // XXX bis: should we keep one phase or 3-phase?
                                "vebus/275/Ac/ActiveIn/L1/F" => {
                                    data.ac_input.frequency = value;
                                }
                                "vebus/275/Ac/ActiveIn/L1/P" => {
                                    data.ac_input.power = value;
                                }
                                "vebus/275/Ac/ActiveIn/L1/V" => {
                                    data.ac_input.voltage = value;
                                }

                                "vebus/275/Ac/Out/L1/F" => {
                                    data.ac_output.frequency = value;
                                }
                                "vebus/275/Ac/Out/L1/P" => {
                                    data.ac_output.power = value;
                                }
                                "vebus/275/Ac/Out/L1/V" => {
                                    data.ac_output.voltage = value;
                                }

                                // XXX where to autodiscover 512??
                                // N/028102353a50/system/0/ActiveBatteryService -> b'{"value": "com.victronenergy.battery/512"}'
                                "battery/512/Dc/0/Temperature" => {
                                    data.battery_dc.temperature = value;
                                }
                                "battery/512/Dc/0/Power" => {
                                    data.battery_dc.power = value;
                                }
                                "battery/512/Dc/0/Current" => {
                                    data.battery_dc.current = value;
                                }
                                "battery/512/Dc/0/Voltage" => {
                                    data.battery_dc.voltage = value;
                                }
                                "battery/512/Soc" => {
                                    data.battery_dc.soc = value;
                                }
                                "battery/512/Soh" => {
                                    data.battery_dc.soh = value;
                                }

                                // XXX how to autodiscover 20??
                                "pvinverter/20/Power" => {
                                    data.solar_power.power = value;
                                }

                                // Schedules
                                /*
                                N/028102353a50/settings/0/Settings/CGwacs/BatteryLife/Schedule/Charge/0/AllowDischarge -> b'{"value": 0}'
                                N/028102353a50/settings/0/Settings/CGwacs/BatteryLife/Schedule/Charge/0/Day -> b'{"value": 7}'
                                N/028102353a50/settings/0/Settings/CGwacs/BatteryLife/Schedule/Charge/0/Duration -> b'{"value": 13800}'
                                N/028102353a50/settings/0/Settings/CGwacs/BatteryLife/Schedule/Charge/0/Soc -> b'{"value": 55}'
                                N/028102353a50/settings/0/Settings/CGwacs/BatteryLife/Schedule/Charge/0/Start -> b'{"value": 7200}'
                                */
                                _ => {}
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error in event loop: {:?}", e);
                        // Handle reconnection or exit
                        break;
                    }
                }
            }
        });

        Ok(Self {
            data,
            client,
            _eventloop_handle,
        })
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

    pub async fn charge_batteries(&self) {
        // XXX this just sets the setpoint. But the power going into the batteries
        // depends on the consupmion of the house
        let topic = "W/028102353a50/settings/0/Settings/CGwacs/AcPowerSetPoint";
        let content = r#"{"value": 1234}"#;
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
