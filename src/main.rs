use crate::key_inspect::get_key;
use crate::peripherals::{
    create_esp_wifi, patch_eventfd, take_gpio12_output, take_gpio13_output, take_gpio9_input,
    ESP_TASK_TIMER_SVR, SYS_LOOP,
};
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::gpio::Pull;
use esp_idf_hal::reset::restart;
use esp_idf_hal::task::block_on;
use esp_idf_svc::wifi::AsyncWifi;
use preludes::*;
use std::{thread, time::Duration};
use wifi::{initial_wifi_connect, prov_check, wifi_prov};

mod app;
mod ble;
mod build_env;
mod crypto;
mod http;
mod key_inspect;
mod mqtt;
mod ntp;
mod peripherals;
mod preludes;
mod proto;
mod wifi;

fn main() {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    patch_eventfd();

    let mut wifi = create_esp_wifi();

    let key = get_key().unwrap();
    if key.is_none() {
        key_inspect::main(wifi).expect("key_inspect_main");
        return;
    }

    let boot_type = get_boot_type().expect("get_boot_type failed");
    info!("boot_type: {:?}", &boot_type);

    if BootType::KeyInspectMode == boot_type {
        key_inspect::main(wifi).expect("key_inspect_main");
        return;
    }

    let mut should_provision = true;
    match prov_check() {
        Ok(p) => {
            if p {
                match wifi.get_configuration() {
                    Ok(c) => {
                        info!("wifi.get_configuration(): {:?}", c);
                        should_provision = false;
                    }
                    Err(e) => {
                        error!("{}", e);
                        restart();
                    }
                };
            }
        }
        Err(e) => {
            error!("prov_check: {}", e);
        }
    }
    if BootType::ForceProvisionMode == boot_type {
        should_provision = true;
    }
    info!("should_provision: {should_provision}");
    if should_provision {
        if let Err(e) = wifi_prov(&mut wifi) {
            error!("wifi_prov: {}", e);
        } else {
            info!("Wi-fi provisioned, now reset.")
        };
        restart();
    } else {
        wifi.stop().unwrap();
        info!("Got Wi-Fi configuration, connecting...");
        let mut wifi = AsyncWifi::wrap(wifi, SYS_LOOP.clone(), ESP_TASK_TIMER_SVR.clone()).unwrap();
        match block_on(initial_wifi_connect(&mut wifi)) {
            Ok(scan_result) => {
                app::main_wrapper(wifi, scan_result).unwrap();
            }
            Err(e) => {
                error!("wifi_connect: {}", e);
                restart();
            }
        }
    }
    info!("Main thread finished, resetting in 3s");
    thread::sleep(Duration::from_secs(3));
    restart();
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum BootType {
    Normal,
    ForceProvisionMode,
    KeyInspectMode,
}

fn get_boot_type() -> Result<BootType> {
    info!("Waiting for boot_type input...");
    let mut led1 = take_gpio12_output();
    let mut led2 = take_gpio13_output();
    let mut button = take_gpio9_input();

    macro_rules! reset_gpio {
        () => {{
            led1.into_disabled()?;
            led2.into_disabled()?;
            button.into_disabled()?;
        }};
    }

    button.set_pull(Pull::Down)?;

    let mut ret = BootType::Normal;

    let mut count = 0;

    let mut high_count = 0;
    let mut low_count = 0;
    loop {
        FreeRtos::delay_ms(10);
        count += 1;

        let r = count % 50;
        if r > 25 {
            led1.set_high()?;
            led2.set_high()?;
        } else {
            led1.set_low()?;
            led2.set_low()?;
        }

        let curr = button.is_low();
        if curr {
            low_count += 1;
        } else {
            low_count = 0;
            high_count += 1;
        }
        if low_count == 0 && ret != BootType::Normal {
            reset_gpio!();
            return Ok(ret);
        }

        if high_count > 600 {
            ret = BootType::Normal;
            reset_gpio!();
            return Ok(ret);
        } else {
            if low_count > 1200 {
                ret = BootType::KeyInspectMode;
                reset_gpio!();
                return Ok(ret);
            }
            if low_count > 200 && low_count < 600 {
                ret = BootType::ForceProvisionMode
            } else {
                ret = BootType::Normal
            }
        }
    }
}
