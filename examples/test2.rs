use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

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
    gx_id: String,
    data: Arc<RwLock<VictronData>>,
}

impl VictronClient {
    pub async fn new(gx_id: String, host: &str, port: u16) -> anyhow::Result<Self> {
        let mut mqttoptions = MqttOptions::new("victron-mqtt-rs", host, port);
        mqttoptions.set_keep_alive(std::time::Duration::from_secs(5));

        let (client, mut connection) = AsyncClient::new(mqttoptions, 10);

        client.publish("N", QoS::AtLeastOnce, false, "30").await?;

        client.subscribe("#", QoS::AtMostOnce).await?;
        client
            .subscribe("N/victron/battery/512/Dc/0/+", QoS::AtMostOnce)
            .await?;
        client
            .subscribe("N/victron/pvinverter/20/Power", QoS::AtMostOnce)
            .await?;

        let data = Arc::new(RwLock::new(VictronData::default()));
        let data_clone = data.clone();

        tokio::spawn(async move {
            while let Ok(event) = connection.poll().await {
                if let Event::Incoming(Packet::Publish(publish)) = event {
                    let topic = publish.topic.clone();
                    let payload = String::from_utf8(publish.payload.to_vec()).unwrap_or_default();
                    let value: Option<f64> = payload.parse().ok();

                    let mut data = data_clone.write().await;

                    println!("Topic: {}, Payload: {}\n", topic, payload);

                    match topic.as_str() {
                        "N/victron/vebus/275/Ac/ActiveLineIn/L1/F" => {
                            data.ac_input.frequency = value;
                        }
                        "N/victron/vebus/275/Ac/ActiveLineIn/L1/P" => {
                            data.ac_input.power = value;
                        }
                        "N/victron/vebus/275/Ac/ActiveLineIn/L1/V" => {
                            data.ac_input.voltage = value;
                        }
                        "N/victron/battery/512/Dc/0/Temperature" => {
                            data.battery_dc.temperature = value;
                        }
                        "N/victron/battery/512/Dc/0/Power" => {
                            data.battery_dc.power = value;
                        }
                        "N/victron/battery/512/Dc/0/Current" => {
                            data.battery_dc.current = value;
                        }
                        "N/victron/battery/512/Dc/0/Voltage" => {
                            data.battery_dc.voltage = value;
                        }
                        "N/victron/pvinverter/20/Power" => {
                            data.solar_power.power = value;
                        }
                        _ => {}
                    }
                }
            }
        });

        Ok(Self { gx_id, data })
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
    let victron = VictronClient::new("".to_string(), "192.168.11.1", 1883)
        .await
        .unwrap();

    // While true, get the ac input data
    loop {
        let ac_input = victron.get_ac_input().await;
        println!("AC Input: {:?}", ac_input);

        let battery_dc = victron.get_battery_dc().await;
        println!("Battery DC: {:?}", battery_dc);

        let solar_power = victron.get_solar_power().await;
        println!("Solar Power: {:?}", solar_power);

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }

    Ok(())
}
