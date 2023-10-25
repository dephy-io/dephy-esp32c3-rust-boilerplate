use crate::app::AppContext;
use crate::crypto::get_eth_address;
use crate::peripherals::{take_gpio12_output, take_gpio13_output};
use crate::preludes::*;
use esp32_nimble::BLEDevice;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::{Output, Pin, PinDriver};
use esp_idf_hal::task::block_on;
use esp_idf_svc::wifi::EspWifi;
use esp_idf_sys::{
    esp_efuse_batch_write_begin, esp_efuse_batch_write_commit, esp_efuse_block_t_EFUSE_BLK_KEY4,
    esp_efuse_desc_t, esp_efuse_get_field_size, esp_efuse_key_block_unused,
    esp_efuse_read_field_blob, esp_efuse_set_write_protect, esp_efuse_write_field_blob,
    esp_fill_random,
};
use k256::SecretKey;
use std::ffi::c_void;
use std::ptr::null;
use std::sync::Arc;
use tokio::sync::Mutex;

pub fn main(mut wifi: EspWifi<'static>) -> Result<()> {
    // Initializing Wi-Fi and BLE to collect entropy for hardware RNG
    wifi.start()?;

    let mut led1 = take_gpio12_output();
    let mut led2 = take_gpio13_output();
    led1.set_high()?;
    led2.set_low()?;

    let name = wifi
        .sta_netif()
        .get_mac()
        .expect("wifi.wifi().sta_netif().get_mac()");
    let name = format!("DePHY_{}", hex::encode(&name));

    let ctx = AppContext { name };
    let ctx = Arc::new(ctx);

    key_loop(ctx.name.clone(), led1, led2)?;

    Ok(())
}

enum KeyInspectStatus {
    Init,
    WaitingForEntropy {
        secs_waited: u64,
    },
    ShouldGenerateKey,
    KeyTaken {
        pubkey_hex: String,
        addr_hex: String,
        secs_waited: u64,
    },
}

fn key_loop<'a, T1: Pin, T2: Pin>(
    name: String,
    mut led1: PinDriver<'a, T1, Output>,
    mut led2: PinDriver<'a, T2, Output>,
) -> Result<()> {
    let ble_device = BLEDevice::take();
    let ble_scan = ble_device.get_scan();
    let ble_scan = ble_scan
        .active_scan(true)
        .filter_duplicates(true)
        .limited(false)
        .interval(100)
        .window(99);

    let _ = block_on(ble_scan.start(1000));

    info!("Key inspect mode!");
    FreeRtos::delay_ms(3000);

    let mut s = KeyInspectStatus::Init;

    loop {
        let wait_secs = match s {
            KeyInspectStatus::Init => {
                if let Some(buf) = get_key()? {
                    let key = SecretKey::from_slice(&buf)?;
                    let key = key.public_key();
                    let pubkey_hex = hex::encode(key.to_sec1_bytes());
                    let addr_hex = get_eth_address(&key.into());
                    s = KeyInspectStatus::KeyTaken {
                        pubkey_hex,
                        addr_hex,
                        secs_waited: 0,
                    };
                } else {
                    s = KeyInspectStatus::WaitingForEntropy { secs_waited: 1 };
                }
                1
            }
            KeyInspectStatus::WaitingForEntropy { secs_waited } => {
                info!(
                    "Have been waiting for {} seconds for entropy...",
                    secs_waited
                );
                let next = secs_waited + 1;

                if next % 2 == 1 {
                    led1.set_low()?;
                    led2.set_high()?;
                } else {
                    led1.set_high()?;
                    led2.set_low()?;
                }

                if next > 3600 {
                    // if next > 2 {
                    s = KeyInspectStatus::ShouldGenerateKey;
                } else {
                    s = KeyInspectStatus::WaitingForEntropy { secs_waited: next };
                }
                1
            }
            KeyInspectStatus::ShouldGenerateKey => {
                info!("Should generate key now!",);
                write_key()?;
                let buf = get_key()?.unwrap();
                let key = SecretKey::from_slice(&buf)?;
                let key = key.public_key();
                let pubkey_hex = hex::encode(key.to_sec1_bytes());
                let addr_hex = get_eth_address(&key.into());

                s = KeyInspectStatus::KeyTaken {
                    pubkey_hex,
                    addr_hex,
                    secs_waited: 0,
                };

                1
            }
            KeyInspectStatus::KeyTaken {
                pubkey_hex,
                addr_hex,
                secs_waited,
            } => {
                if secs_waited % 10 == 0 {
                    info!("name: {}", &name);
                    info!("pubkey_hex: {}", pubkey_hex.as_str());
                    info!("addr_hex: {}", addr_hex.as_str());

                    println!(
                        "\n\n{{\"device_name\":\"{}\",\"pubkey_hex\":\"{}\",\"addr_hex\":\"{}\"}}\n\n", 
                        &name,
                        pubkey_hex.as_str(),
                        addr_hex.as_str()
                    );
                }

                if secs_waited % 2 == 1 {
                    led1.set_high()?;
                    led2.set_high()?;
                } else {
                    led1.set_low()?;
                    led2.set_low()?;
                }

                s = KeyInspectStatus::KeyTaken {
                    pubkey_hex,
                    addr_hex,
                    secs_waited: secs_waited + 1,
                };

                1
            }
        };
        FreeRtos::delay_ms(wait_secs * 1000);
    }
}

pub fn get_key() -> Result<Option<[u8; 32]>> {
    unsafe {
        if esp_efuse_key_block_unused(esp_efuse_block_t_EFUSE_BLK_KEY4) {
            info!("esp_efuse_block_t_EFUSE_BLK_KEY4 not used.");
            return Ok(None);
        }
        let mut desc4 = esp_efuse_desc_t::default();
        desc4.set_efuse_block(esp_efuse_block_t_EFUSE_BLK_KEY4);
        desc4.bit_start = 0;
        desc4.bit_count = 256;

        let mut desc4 = [&desc4 as *const esp_efuse_desc_t, null()];
        let desc4 = desc4.as_mut_ptr();

        let size4 = esp_efuse_get_field_size(desc4);
        info!("size4: {}", size4);
        if size4 == 0 {
            return Ok(None);
        }
        let mut buf = [0u8; 32];
        esp!(esp_efuse_read_field_blob(
            desc4,
            buf.as_mut_ptr() as *mut c_void,
            256
        ))?;
        return Ok(Some(buf));
    }
}

fn write_key() -> Result<()> {
    unsafe {
        let mut buf = [0u8; 32];
        esp_fill_random(buf.as_mut_ptr() as *mut c_void, 32);

        let mut desc4 = esp_efuse_desc_t::default();
        desc4.set_efuse_block(esp_efuse_block_t_EFUSE_BLK_KEY4);
        desc4.bit_start = 0;
        desc4.bit_count = 256;

        let mut desc4 = [&desc4 as *const esp_efuse_desc_t, null()];
        let desc4 = desc4.as_mut_ptr();

        esp!(esp_efuse_batch_write_begin())?;
        esp!(esp_efuse_write_field_blob(
            desc4,
            buf.as_ptr() as *const c_void,
            256
        ))?;
        esp!(esp_efuse_set_write_protect(
            esp_efuse_block_t_EFUSE_BLK_KEY4
        ))?;
        esp!(esp_efuse_batch_write_commit())?;
        info!("Random key has been written to eFuse!");
    }
    Ok(())
}
