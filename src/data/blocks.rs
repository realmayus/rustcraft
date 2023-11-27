use serde_json::{Map, Value};
use tokio::fs;


// pub(crate) async fn parse_blocks() {
//     let registry_json = fs::read_to_string("assets/registry.json").await.unwrap();
//     let registry_json = serde_json::from_str::<Value>(&registry_json).unwrap();
//     for (ident, value) in registry_json.as_object().unwrap() {
//         parse_block(value.as_object().unwrap())
//     }
// }