use std::time::Duration;

use victron_gx::VictronGx;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let serial_number = "028102353a50";
    let client = VictronGx::new("victron_client", "192.168.11.1", 1883, serial_number).await?;

    client.send_mqtt().await?;

    client.ess_set_setpoint(130.0).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;
    Ok(())
}
