use std::time::Duration;

use victron_gx::VictronGx;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        //        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let serial_number = "028102353a50";
    let client = VictronGx::new("victron_client", "192.168.11.1", 1883, serial_number).await?;

    client.send_mqtt().await?;

    // Periodically print the data
    loop {
        /*
        let ac_input = client.get_ac_input().await;
        println!("AC Input: {:?}", ac_input);

        let ac_output = client.get_ac_output().await;
        println!("AC Output: {:?}", ac_output);

        let batteries = client.get_batteries_dc().await;
        println!("Battery DC: {:?}", batteries);
        */

        let inverters = client.get_pv_inverters().await;
        println!("Solar Power: {:?}", inverters);
        let ess = client.get_ess().await;
        println!("ESS: {:?}", ess);

        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}
