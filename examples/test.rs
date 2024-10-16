use victron_gx::GxDevice;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gx = GxDevice::new("192.168.11.1:502".parse().unwrap());

    //gx.get_modbus_value_vecu16(228, 23, 1).await;
    let ip = gx.get_input_power().await?;
    println!("Input Power: {:.2} W", ip);

    let op = gx.get_output_power().await?;
    println!("Output Power: {:.2} W", op);

    let batt_power = gx.get_battery_power().await?;
    println!("Battery Power: {:.2} W", batt_power);

    let battery_soc = gx.get_battery_soc().await?;
    println!("Battery SOC: {:.2} %", battery_soc);

    Ok(())
}
