use crate::preludes::*;
use embedded_svc::http::client::Client as HttpClient;
use embedded_svc::http::Method;
use embedded_svc::io::Write;
use embedded_svc::utils::io;
use esp_idf_svc::http::client::{
    Configuration as HttpConfiguration, EspHttpConnection, FollowRedirectsPolicy,
};
use esp_idf_sys::esp_crt_bundle_attach;
use std::vec::Vec;

static COMMON_HEADERS: &'static [(&'static str, &'static str); 3] = &[
    ("User-Agent", "curl/8.1.2"),
    ("accept", "*/*"),
    ("content-type", "application/x-dephy"),
];

// lazy_static! {
//     static ref HTTP_CLIENT: Arc<Mutex<HttpClient<EspHttpConnection>>> =
//         Arc::new(Mutex::new(create_default_client().unwrap()));
// }

pub fn create_default_client() -> Result<HttpClient<EspHttpConnection>> {
    let http = HttpConfiguration {
        buffer_size: None,
        buffer_size_tx: None,
        timeout: Some(Duration::from_secs(18)),
        follow_redirects_policy: FollowRedirectsPolicy::FollowGetHead,
        client_certificate: None,
        private_key: None,
        use_global_ca_store: true,
        crt_bundle_attach: Some(esp_crt_bundle_attach),
    };
    let http = EspHttpConnection::new(&http)?;
    Ok(HttpClient::wrap(http))
}

pub fn request_text<'a>(
    url: &str,
    method: Option<Method>,
    user_headers: &[(&str, &str)],
    body_buf: Option<&'a [u8]>,
) -> Result<String> {
    let (buf, bytes_read) = request(url, method, user_headers, body_buf)?;
    let buf = &buf[..bytes_read];
    let ret = std::str::from_utf8(&buf)?;
    debug!(
        "Response body (truncated to {} bytes): {:?}",
        bytes_read, ret
    );
    Ok(ret.to_string())
}

fn request<'a>(
    url: &'a str,
    method: Option<Method>,
    user_headers: &[(&'a str, &'a str)],
    body_buf: Option<&'a [u8]>,
) -> Result<([u8; 2048], usize)> {
    let mut headers = Vec::new();
    headers.extend(COMMON_HEADERS.clone().into_iter());

    let mut len = "0".to_string();
    if let Some(buf) = body_buf {
        len = buf.len().to_string();
    }
    let e = [("content-length", &len[..])];
    headers.extend(e.clone().into_iter());

    headers.extend_from_slice(user_headers);

    let h = headers.clone();
    let mut client = create_default_client()?;
    let mut request = client.request(
        if let Some(m) = method { m } else { Method::Get },
        url,
        h.as_slice(),
    )?;
    debug!("-> GET {}", url);
    if let Some(buf) = body_buf {
        request.write_all(buf)?;
        request.flush()?;
    }
    let mut response = request.submit()?;

    // Process response
    let status = response.status();
    debug!("<- {}", status);
    let (_headers, mut body) = response.split();
    let mut buf = [0u8; 2048];
    let bytes_read = io::try_read_full(&mut body, &mut buf).map_err(|e| e.0)?;
    debug!("Read {} bytes", bytes_read);

    Ok((buf, bytes_read))
}
