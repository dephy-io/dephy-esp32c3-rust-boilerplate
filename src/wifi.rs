use crate::ntp::ntp_sync;
use crate::peripherals::{take_gpio12_output, take_gpio13_output};
use crate::preludes::*;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_svc::wifi::{AsyncWifi, EspWifi};
use esp_idf_sys::{
    esp_wifi_clear_ap_list, wifi_prov_event_handler_t, wifi_prov_mgr_config_t,
    wifi_prov_mgr_deinit, wifi_prov_mgr_init, wifi_prov_mgr_is_provisioned,
    wifi_prov_mgr_start_provisioning, wifi_prov_mgr_wait, wifi_prov_scheme_ble,
    wifi_prov_security_WIFI_PROV_SECURITY_1,
};
use std::{
    ffi::{c_void, CString},
    ptr::null_mut,
    thread,
};
use tokio::time::sleep;

fn prov_led_blink() -> Result<()> {
    let mut count = 0;
    let mut led1 = take_gpio12_output();
    let mut led2 = take_gpio13_output();

    loop {
        FreeRtos::delay_ms(10);
        count += 1;
        let r = count % 30;
        if r > 15 {
            led1.set_high()?;
            led2.set_low()?;
        } else {
            led1.set_low()?;
            led2.set_high()?;
        }
    }
}

pub fn wifi_prov(wifi: &mut EspWifi) -> Result<()> {
    let pop = CString::new("abcd1234")?;
    let pop_ptr = pop.as_ptr() as *const c_void;

    thread::spawn(|| prov_led_blink());
    wifi.start()?;

    unsafe {
        let config = wifi_prov_mgr_config_t {
            scheme: wifi_prov_scheme_ble,
            scheme_event_handler: wifi_prov_event_handler_t {
                event_cb: None,
                user_data: null_mut(),
            },
            app_event_handler: wifi_prov_event_handler_t {
                event_cb: None,
                user_data: null_mut(),
            },
        };
        esp!(wifi_prov_mgr_init(config))?;
        let name = wifi.sta_netif().get_mac()?;
        let name = format!("PROV_DePHY_{}", hex::encode(&name));
        let name = CString::new(name)?;

        esp!(wifi_prov_mgr_start_provisioning(
            wifi_prov_security_WIFI_PROV_SECURITY_1,
            pop_ptr,
            name.as_ptr(),
            null_mut(),
        ))?;
        wifi_prov_mgr_wait();
        wifi_prov_mgr_deinit();

        Ok(())
    }
}

pub fn prov_check() -> Result<bool> {
    unsafe {
        let mut provisioned = false;
        wifi_prov_mgr_is_provisioned(&mut provisioned);
        info!("provisioned: {provisioned}");
        Ok(provisioned)
    }
}

pub async fn initial_wifi_connect(wifi: &mut AsyncWifi<EspWifi<'static>>) -> Result<MacList> {
    wifi.start().await?;

    let scan_result = wifi_scan(wifi).await?;

    wifi.connect().await?;
    wifi.wait_netif_up().await?;

    info!("Connected to Wi-fi, now trying setting time from ntp.");
    ntp_sync()?;

    Ok(scan_result)
}

pub async fn app_wifi_loop(mut wifi: AsyncWifi<EspWifi<'static>>) -> Result<()> {
    let mut count = 0u8;
    let mut fail_count = 0u8;

    loop {
        sleep(Duration::from_secs(10)).await;
        count += 1;

        if count == 8 {
            count = 0;
            // Wi-Fi driver will fault when trying scanning while connected to AP
            // if let Err(e) = wifi_scan(&mut wifi).await {
            //     error!("wifi_scan: {}", e);
            // }

            if fail_count > 0 {
                info!("Network failure detected, try re-connecting...");
                wifi.disconnect().await?;
                wifi.stop().await?;
                initial_wifi_connect(&mut wifi).await?;
                info!("Connected to Wi-fi, now trying setting time from ntp.");
                ntp_sync()?;
            }
        }

        if count >= 6 && fail_count > 0 {
            if let Err(e) = ntp_sync() {
                error!("ntp_sync: {}", e);
                fail_count += 1;
            } else {
                fail_count = 0;
            };
        }
    }
}

pub async fn wifi_scan<'a>(wifi: &'a mut AsyncWifi<EspWifi<'static>>) -> Result<MacList> {
    esp!(unsafe { esp_wifi_clear_ap_list() })?;
    let (scan, _) = wifi.scan_n::<32>().await?;
    let mut ret: HeaplessVec<_, 32> = HeaplessVec::new();
    for ap in scan.into_iter() {
        ret.push(ap.bssid).expect("buf.push");
    }
    info!("wifi_scan: {:?}", ret);
    Ok(ret)
}

pub type MacList = HeaplessVec<[u8; 6], 32>;
