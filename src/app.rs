use crate::ble;
use crate::crypto::{create_signed_message, MY_ADDRESS_STRING};
use crate::http::request_text;
use crate::peripherals::{
    create_timer_driver_00, take_gpio12_output, take_gpio13_output, take_i2c,
};
use crate::preludes::*;
use crate::wifi::{app_wifi_loop, MacList};
use chrono::Utc;
use embedded_svc::http::Method;
use esp32_nimble::BLEDevice;
use esp_idf_svc::wifi::{AsyncWifi, EspWifi};
use std::sync::Arc;
use tokio::time::sleep;

pub static RESPONSE_JSON_OK: &'static str = "{\"ok\":true}";
pub static CELCIUS_CONVERSION: f32 = 0.00390625;

#[derive(Clone)]
pub struct AppContext {
    pub name: String,
}

pub fn main_wrapper(wifi: AsyncWifi<EspWifi<'static>>, wifi_scan_result: MacList) -> Result<()> {
    let mut led1 = take_gpio12_output();
    let mut led2 = take_gpio13_output();
    led1.set_low()?;
    led2.set_low()?;

    let name = wifi.wifi().sta_netif().get_mac()?;
    let name = format!("DePHY_{}", hex::encode(&name));

    let ctx = AppContext { name };
    let ctx = Arc::new(ctx);

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async move {
            tokio::select! {
                ret = ble_task(ctx.clone()) => {
                    if let Err(e) = ret {
                        error!("ble_task: {}", e)
                    }
                }
                ret = main(ctx.clone()) => {
                    if let Err(e) = ret {
                        error!("main: {}", e)
                    }
                }
                ret = app_wifi_loop(wifi) => {
                    if let Err(e) = ret {
                        error!("app_wifi_loop: {}", e)
                    }
                }
            }
        });

    Ok(())
}

async fn main(ctx: Arc<AppContext>) -> Result<()> {
    // use the timer when you need accurate timing
    let _td = create_timer_driver_00();

    let mut i2c = take_i2c();
    let addr = 0x45u8;
    let i2c_w_read = [0xccu8, 0x44u8];
    let mut i2c_buf = [0u8; 3];

    // It must fail here on first boot, I don't know why.
    if let Err(e) = i2c.read(addr, &mut i2c_buf, 50) {
        info!("i2c.read: {}", e);
    };

    info!("My address: 0x{}", MY_ADDRESS_STRING.as_str());

    let mut cycle_count = 0u8;
    let mut temperature = 0f32;

    loop {
        // I2C example getting temperature from Mysentech M117B sensor
        if let Err(e) = i2c.write_read(addr, &i2c_w_read, &mut i2c_buf, 50) {
            error!("i2c.write_read: {}", e);
        } else {
            let temp = u16::from_be_bytes(i2c_buf[0..2].try_into().unwrap());
            let temp = (temp as i16) as f32 * CELCIUS_CONVERSION;
            let temp = 40.0 + temp;
            temperature = temp;
            info!("Temp: {}", temperature);
        }

        if cycle_count >= 30 {
            if let Err(e) = publish_message(ctx.clone(), temperature).await {
                error!("publish_message: {}", e);
                info!("retrying in next cycle.")
            } else {
                cycle_count = 0;
            }
        }

        cycle_count += 1;
        sleep(Duration::from_secs(APP_SEND_LOOP_DURATION)).await;
    }
}

async fn publish_message(ctx: Arc<AppContext>, temp: f32) -> Result<()> {
    let body = format!("{},{}", ctx.name.as_str(), temp);
    let body = body.as_bytes().to_vec();
    let body = create_signed_message(body, None)?;
    let body = body.encode_to_vec();

    let now = Utc::now();
    let now = now.to_rfc2822();

    match request_text(
        DEPHY_ENDPOINT_HTTP,
        Some(Method::Post),
        &[],
        Some(body.as_slice()),
    ) {
        Ok(ret) => {
            if ret == RESPONSE_JSON_OK {
                info!("[{}] Published message", now.as_str());
            } else {
                bail!("[{}] failed to publish message: {}", now.as_str(), ret)
            }
        }
        Err(e) => {
            bail!("[{}] failed to publish message: {}", now.as_str(), e)
        }
    }
    Ok(())
}

pub async fn ble_task(ctx: Arc<AppContext>) -> Result<()> {
    let ble_device = BLEDevice::take();
    let ble_server = ble_device.get_server();
    let ble_advertising = ble_device.get_advertising();

    let name = ctx.name.clone();
    let name = name.as_str();

    ble::ble_advertise_task(name, ble_server, ble_advertising).await;
    Ok(())
}
