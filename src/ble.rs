use crate::preludes::*;
use esp32_nimble::utilities::BleUuid;
use esp32_nimble::{BLEAdvertising, BLEScan, BLEServer, NimbleProperties};
use lazy_static::lazy_static;
use tokio::sync::oneshot;
use tokio::time::sleep;

pub static UUID_BLE_SERVICE_STR: &'static str = "io.dephy.ble"; // up-to 16 bytes
pub static UUID_BLE_UPTIME_CHARA_STR: &'static str = "uptime"; // up-to 16 bytes

lazy_static! {
    pub static ref UUID_BLE_SERVICE: BleUuid = str_to_uuid(UUID_BLE_SERVICE_STR);
    pub static ref UUID_BLE_UPTIME_CHARA: BleUuid = str_to_uuid(UUID_BLE_UPTIME_CHARA_STR);
}

pub fn str_to_uuid(s: &str) -> BleUuid {
    let mut arr = [0u8; 16];
    for (idx, char) in s.as_bytes().iter().enumerate() {
        if idx < 16 {
            arr[idx] = *char
        } else {
            warn!("uuid string is longer than 16 bytes!");
            break;
        }
    }
    BleUuid::from_uuid128(arr)
}

pub async fn ble_advertise_task(
    name: &str,
    server: &mut BLEServer,
    advertising: &mut BLEAdvertising,
) -> () {
    server.on_connect(|server, desc| {
        server
            .update_conn_params(desc.conn_handle, 24, 48, 0, 60)
            .expect("server.update_conn_params");
    });
    //    server.on_disconnect(|desc, reason| {
    //        info!("Client disconnected ({:X})", reason);
    //    });
    let service = server.create_service(UUID_BLE_SERVICE.clone());

    let notifying_characteristic = service.lock().create_characteristic(
        UUID_BLE_UPTIME_CHARA.clone(),
        NimbleProperties::READ | NimbleProperties::NOTIFY,
    );
    notifying_characteristic.lock().set_value(b"uptime: 0");

    advertising
        .name(name)
        .add_service_uuid(UUID_BLE_SERVICE.clone());

    advertising.start().expect("ble_advertising.start()");

    let mut counter: u128 = 0;

    loop {
        sleep(Duration::from_secs(1)).await;

        let mut guard = notifying_characteristic.lock();
        guard
            .set_value(format!("uptime: {counter}").as_bytes())
            .notify();
        drop(guard);

        counter += 1;
    }
}

pub async fn do_ble_scan(ble_scan: &mut BLEScan) -> Result<Vec<Vec<u8>>> {
    let (tx, rx) = oneshot::channel::<()>();
    let mut tx = Some(tx);

    ble_scan
        .active_scan(true)
        .filter_duplicates(true)
        .limited(false)
        .interval(100)
        .window(99)
        .on_completed(move || {
            let tx = tx.take();
            if let Some(tx) = tx {
                let _ = tx.send(());
            }
        });
    ble_scan
        .start(10000)
        .await
        .map_err(|e| anyhow!("ble_scan.start: {:?}", e))?;
    let _ = tokio::select! {
        _ = sleep(Duration::from_secs(15)) => {
            bail!("ble scan timed out!");
        }
        _ = rx => {
            info!("Scan finished");
        }
    };
    let result = ble_scan
        .get_results()
        .map(|i| {
            let addr = format!("{}", i.addr());
            addr.split(":")
                .map(|s| u8::from_str_radix(s, 16).unwrap_or(16))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    Ok(result)
}
