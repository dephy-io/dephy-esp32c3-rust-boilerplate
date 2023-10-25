// use crate::preludes::*;
// use bytes::Bytes;
// use mqtt_async_client::client::{Client, Publish};
// use tokio::sync::mpsc;
//
// lazy_static::lazy_static! {
//     pub static ref BROKER_URL: String = "mqtt://demo-edge.dephy.io".to_string();
//     pub static ref PUBLISH_TOPIC: String = "/dephy/signed_message".to_string();
//     pub static ref SUBSCR_TOPICS: Vec<&'static str> = vec!["/dephy/signed_message"];
// }
//
// pub async fn mqtt_task(
//     mut client: Client,
//     mut mqtt_send_rx: mpsc::UnboundedReceiver<Bytes>,
// ) -> Result<()> {
//     // tokio::join!();
//     // let fut_tx = send_loop(&client, mqtt_send_rx);
//     // let fut_rx = read_subscr_loop(client);
//
//     Ok(())
// }
