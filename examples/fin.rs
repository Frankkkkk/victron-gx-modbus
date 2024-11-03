use std::time::Duration;

use victron_gx::VictronGx;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let serial_number = "028102353a50";
    let client = VictronGx::new("victron_client", "192.168.11.1", 1883, serial_number).await?;

    client.send_mqtt().await?;

    // Periodically print the data
    loop {
        let ac_input = client.get_ac_input().await;
        println!("AC Input: {:?}", ac_input);

        let ac_output = client.get_ac_output().await;
        println!("AC Output: {:?}", ac_output);

        let battery_dc = client.get_battery_dc().await;
        println!("Battery DC: {:?}", battery_dc);

        let solar_power = client.get_solar_power().await;
        println!("Solar Power: {:?}", solar_power);

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
