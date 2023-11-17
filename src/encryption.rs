use std::fmt::format;
use std::sync::Arc;
use log::debug;
use num_bigint::BigInt;
use openssl::sha::Sha1;
use serde::Deserialize;

use crate::Assets;
use crate::connection::Connection;

#[derive(Debug, Deserialize)]
struct Response {
    id: String,
    name: String,
}

async fn request(username: String, server_id: String) -> Result<(), String> {
    let url = format!("https://sessionserver.mojang.com/session/minecraft/hasJoined?username={}&serverId={}", username, server_id);
    // Send a GET request to the given url, and then deserialize the JSON response body as an `Response`.
    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("Couldn't send request: {}", e))?
        .json::<Response>()
        .await
        .map_err(|e| format!("Couldn't deserialize response: {}", e))?;

    debug!("Response GET {:?}", response);
    Ok(())
}

pub(crate) async fn encrypt(verify_token_plain: Vec<u8>, shared_secret_plain: Vec<u8>, assets: Arc<Assets>, connection: &mut Connection) -> Result<(), String> {
    assert_eq!(verify_token_plain[0..4], connection.verify_token);

    let mut sha = Sha1::new();
    sha.update("".as_bytes());
    sha.update(&shared_secret_plain);
    sha.update(&assets.pub_key);
    let hex = BigInt::from_signed_bytes_be(&sha.finish()).to_str_radix(16);
    request(connection.player.clone(), hex).await?;
    Ok(())
}