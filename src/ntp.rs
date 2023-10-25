use crate::preludes::*;
use byteorder::{BigEndian, ReadBytesExt};
use esp_idf_sys::{settimeofday, time_t, timeval};
use std::io::{Cursor, Seek, SeekFrom};
use std::net::UdpSocket;
use std::ptr::null;
use std::thread;
use std::time::Duration;

fn unpack_ntp_data(buffer: &[u8; 48]) -> u64 {
    let mut reader = Cursor::new(buffer);
    reader.seek(SeekFrom::Current(40)).unwrap();
    let ntp_second = reader.read_u32::<BigEndian>().unwrap();
    u64::from(ntp_second)
}

pub fn request(client: &UdpSocket, server: &str) -> Result<u64> {
    client.connect(format!("{server}:123"))?;
    let mut request_data = vec![0; 48];
    request_data[0] = 0x1b;
    client.send(&request_data)?;
    let mut buf = [0; 48];
    client.recv(&mut buf)?;
    let ntp_second = unpack_ntp_data(&buf);
    let unix_second = ntp_second - 2208988800;

    Ok(unix_second)
}

pub fn ntp_sync() -> Result<()> {
    let client = UdpSocket::bind("0.0.0.0:0")?;
    client.set_read_timeout(Some(Duration::from_secs(3)))?;

    if let Some(res) = {
        let mut res = None;
        for s in NTP_SERVERS.into_iter() {
            info!("Trying to sync time with {}...", s);
            match request(&client, s) {
                Ok(t) => {
                    res = Some(t);
                    break;
                }
                Err(e) => {
                    // no more processes means timed-out
                    error!("Failed to sync time with {}: {}", s, e);
                }
            }
        }
        res
    } {
        unsafe {
            let time = timeval {
                tv_sec: res as time_t,
                tv_usec: 0,
            };
            settimeofday(&time, null());
        }
        info!("Got time: {:?}", &res);
    } else {
        error!("Failed to sync time from NTP servers, resetting in 3s...");
        thread::sleep(Duration::from_secs(3));
    }

    drop(client);
    Ok(())
}

static NTP_SERVERS: [&str; 5] = [
    "time.apple.com",
    "ntp.aliyun.com",
    "time.windows.com",
    "1.1.1.1",
    "time-nw.nist.gov",
];
