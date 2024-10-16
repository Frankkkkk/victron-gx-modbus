use std::{fmt::Error, net::SocketAddr, net::TcpStream};

use tokio_modbus::{
    client::{tcp, Context},
    Slave,
};

use byteorder::{BigEndian, ByteOrder};

pub struct GxDevice {
    socket_addr: SocketAddr,
}

const GX_VE_DEVICE_0: u8 = 100;
const GX_MII_VE_BUS: u8 = 228;
const GX_CAN_BUS_BMS: u8 = 225;

impl GxDevice {
    pub fn new(socket_addr: SocketAddr) -> Self {
        GxDevice { socket_addr }
    }

    pub async fn get_modbus_i16(
        &self,
        device_id: u8,
        address: u16,
        scale_factor: f32,
    ) -> Result<f32, anyhow::Error> {
        use tokio_modbus::prelude::*;

        let slave = Slave(device_id);

        let mut ctx = tcp::connect_slave(self.socket_addr, slave).await.unwrap();

        let x = ctx.read_holding_registers(address, 1).await.unwrap();

        if let Ok(x) = x {
            ctx.disconnect().await.unwrap();
            let val = x[0];
            let real_val = BigEndian::read_i16(&val.to_be_bytes());

            return Ok(real_val as f32 / scale_factor);
        }
        return Err(anyhow::Error::msg("Error reading modbus"));
    }

    pub async fn get_modbus_u16(
        &self,
        device_id: u8,
        address: u16,
        scale_factor: f32,
    ) -> Result<f32, anyhow::Error> {
        use tokio_modbus::prelude::*;

        let slave = Slave(device_id);

        let mut ctx = tcp::connect_slave(self.socket_addr, slave).await.unwrap();

        let x = ctx.read_holding_registers(address, 1).await.unwrap();

        if let Ok(x) = x {
            ctx.disconnect().await.unwrap();
            return Ok(x[0] as f32 / scale_factor);
        }
        return Err(anyhow::Error::msg("Error reading modbus"));
    }

    pub async fn get_input_power(&self) -> Result<f32, anyhow::Error> {
        self.get_modbus_i16(GX_MII_VE_BUS, 12, 0.1).await
    }

    pub async fn get_output_power(&self) -> Result<f32, anyhow::Error> {
        self.get_modbus_i16(GX_MII_VE_BUS, 23, 0.1).await
    }

    pub async fn get_battery_power(&self) -> Result<f32, anyhow::Error> {
        self.get_modbus_i16(GX_CAN_BUS_BMS, 258, 1.).await
    }
    pub async fn get_battery_soc(&self) -> Result<f32, anyhow::Error> {
        self.get_modbus_u16(GX_CAN_BUS_BMS, 266, 10.).await
    }

    pub async fn get_solar_power(&self) -> Result<f32, anyhow::Error> {
        // Basically the difference between input and output power (- inverters if seen by victron)
        self.get_modbus_u16(GX_VE_DEVICE_0, 866, 1.).await
    }

    pub async fn get_setpoint(&self) -> Result<f32, anyhow::Error> {
        self.get_modbus_i16(GX_VE_DEVICE_0, 2700, 1.).await
    }
}
