use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ACInput {
    pub voltage: Option<f64>,
    pub power: Option<f64>,
    pub frequency: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BatteryDC {
    pub temperature: Option<f64>,
    pub power: Option<f64>,
    pub current: Option<f64>,
    pub voltage: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SolarPower {
    pub power: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct VictronData {
    pub ac_input: ACInput,
    pub battery_dc: BatteryDC,
    pub solar_power: SolarPower,
}

pub struct VictronClient {
    data: Arc<RwLock<VictronData>>,
    _client: AsyncClient,
}

impl VictronClient {
    pub async fn new(host: &str, port: u16, serial_number: &str) -> anyhow::Result<Self> {
        let mut mqttoptions = MqttOptions::new("victron-gx-rs", host, port);
        mqttoptions.set_keep_alive(std::time::Duration::from_secs(30));

        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

        // Publish keepalive message
        let keepalive_topic = format!("N/{}/keepalive", serial_number);
        client
            .publish(&keepalive_topic, QoS::AtLeastOnce, false, "30")
            .await?;

        // Subscribe to topics with the serial number
        let topics = vec![
            format!("N/{}/#", serial_number),
            format!("N/{}/vebus/275/Ac/ActiveLineIn/L1/F", serial_number),
            format!("N/{}/vebus/275/Ac/ActiveLineIn/L1/P", serial_number),
            format!("N/{}/vebus/275/Ac/ActiveLineIn/L1/V", serial_number),
            format!("N/{}/battery/512/Dc/0/Temperature", serial_number),
            format!("N/{}/battery/512/Dc/0/Power", serial_number),
            format!("N/{}/battery/512/Dc/0/Current", serial_number),
            format!("N/{}/battery/512/Dc/0/Voltage", serial_number),
            format!("N/{}/pvinverter/20/Power", serial_number),
        ];

        for topic in &topics {
            client.subscribe(topic, QoS::AtMostOnce).await?;
        }

        let data = Arc::new(RwLock::new(VictronData::default()));
        let data_clone = data.clone();
        let serial_number = serial_number.to_string();

        println!("Subscribed to topics: {:?}", topics);

        task::spawn(async move {
            while let Ok(event) = eventloop.poll().await {
                if let Event::Incoming(Packet::Publish(publish)) = event {
                    let topic = publish.topic.clone();
                    let payload = String::from_utf8_lossy(&publish.payload).to_string();
                    let value: Option<f64> = payload.parse().ok();

                    let mut data = data_clone.write().await;

                    // Extract the part after the serial number
                    let topic_suffix = topic
                        .strip_prefix(&format!("N/{}/", serial_number))
                        .unwrap_or("");

                    println!("Topic: {}, Payload: {}\n", topic, payload);
                    println!("Topic Suffix: {}\n", topic_suffix);

                    match topic_suffix {
                        "vebus/275/Ac/ActiveIn/L1/F" => {
                            data.ac_input.frequency = value;
                        }
                        "vebus/275/Ac/ActiveIn/L1/P" => {
                            data.ac_input.power = value;
                        }
                        "vebus/275/Ac/ActiveIn/L1/V" => {
                            data.ac_input.voltage = value;
                        }
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
                        "pvinverter/20/Power" => {
                            data.solar_power.power = value;
                        }
                        _ => {}
                    }
                }
            }
        });

        println!("Returning VictronClient");

        Ok(Self {
            data,
            _client: client,
        })
    }

    pub async fn get_ac_input(&self) -> ACInput {
        self.data.read().await.ac_input.clone()
    }

    pub async fn get_battery_dc(&self) -> BatteryDC {
        self.data.read().await.battery_dc.clone()
    }

    pub async fn get_solar_power(&self) -> SolarPower {
        self.data.read().await.solar_power.clone()
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let victron = VictronClient::new("192.168.11.1", 1883, "028102353a50")
        .await
        .unwrap();
    println!("Will loop");

    // While true, get the ac input data
    loop {
        println!("Looping...");
        let all = victron.data.read().await;
        println!("All: {:?}", all);
        /*
        let ac_input = victron.get_ac_input().await;
        println!("AC Input: {:?}", ac_input);

        let battery_dc = victron.get_battery_dc().await;
        println!("Battery DC: {:?}", battery_dc);

        let solar_power = victron.get_solar_power().await;
        println!("Solar Power: {:?}", solar_power);
        */

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    Ok(())
}
