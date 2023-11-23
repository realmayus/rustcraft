use std::sync::Arc;

use log::debug;
use num_bigint::BigInt;
use serde::Deserialize;
use sha1::digest::FixedOutputReset;
use sha1::Digest;
use sha1::Sha1;
use uuid::Uuid;
use crate::Assets;

use crate::connection::Connection;

#[derive(Debug, Deserialize)]
struct Response {
    id: String,
    name: String,
}

async fn authenticate_session(username: String, server_id: String) -> Result<Uuid, String> {
    let url = format!(
        "https://sessionserver.mojang.com/session/minecraft/hasJoined?username={}&serverId={}",
        username, server_id
    );
    // Send a GET request to the given url, and then deserialize the JSON response body as an `Response`.
    let body = reqwest::get(&url)
        .await
        .map_err(|e| format!("Couldn't send request: {}", e))?;

    //print response body, then try to deserialize it as a json <Response>
    let response = body
        .json::<Response>()
        .await
        .map_err(|e| format!("Couldn't deserialize response: {}, {:?}", e, e.url()))?;

    debug!("Response GET {:?}", response);
    Ok(Uuid::parse_str(&response.id).unwrap())
}

pub(crate) async fn encrypt(
    shared_secret_plain: &[u8],
    assets: Arc<Assets>,
    connection: &mut Connection,
) -> Result<Uuid, String> {
    let hash = compute_server_hash(assets.clone(), shared_secret_plain);
    let uuid = authenticate_session(connection.username.clone(), hash).await?;
    debug!("UUID: {:?}", uuid);
    Ok(uuid)
}

fn compute_server_hash(assets: Arc<Assets>, shared_secret: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(b""); // server ID - always empty
    hasher.update(&shared_secret);
    let key = assets.pub_key.clone();
    hasher.update(&key);
    hexdigest(hasher.finalize_fixed_reset().as_slice())
}

// Non-standard hex digest used by Minecraft.
fn hexdigest(bytes: &[u8]) -> String {
    let bigint = BigInt::from_signed_bytes_be(bytes);
    let is_negative = bigint < BigInt::from(0);
    let bigint = if is_negative { -bigint } else { bigint };
    let res = format!("{:x}", &bigint).trim_start_matches("0").into();
    if is_negative {
        format!("-{}", res)
    } else {
        res
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_digest1() {
        let mut hasher = Sha1::new();
        hasher.update(b"Notch");
        let hash = hexdigest(hasher.finalize_fixed_reset().as_slice());
        assert_eq!(hash, "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48");
    }

    #[test]
    fn test_digest2() {
        let mut hasher = Sha1::new();
        hasher.update(b"jeb_");
        let hash = hexdigest(hasher.finalize_fixed_reset().as_slice());
        assert_eq!(hash, "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1");
    }

    #[test]
    fn test_digest3() {
        let mut hasher = Sha1::new();
        hasher.update(b"simon");
        let hash = hexdigest(hasher.finalize_fixed_reset().as_slice());
        assert_eq!(hash, "88e16a1019277b15d58faf0541e11910eb756f6");
    }
}
