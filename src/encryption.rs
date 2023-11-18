use std::sync::Arc;
use log::debug;
use num_bigint::BigInt;
use serde::Deserialize;
use crate::Assets;
use crate::connection::Connection;
use sha1::Sha1;
use sha1::Digest;
use sha1::digest::{FixedOutput, FixedOutputReset};

#[derive(Debug, Deserialize)]
struct Response {
    id: String,
    name: String,
}

async fn request(username: String, server_id: String) -> Result<(), String> {
    let url = format!("https://sessionserver.mojang.com/session/minecraft/hasJoined?username={}&serverId={}&ip=192.168.178.1", username, server_id);
    // Send a GET request to the given url, and then deserialize the JSON response body as an `Response`.
    let body = reqwest::get(&url)
        .await
        .map_err(|e| format!("Couldn't send request: {}", e))?;

    //print response body, then try to deserialize it as a json <Response>
    debug!("Response: {:?}, {url}", body.text().await);
    // let response = body
    //     .json::<Response>()
    //     .await
    //     .map_err(|e| format!("Couldn't deserialize response: {}, {:?}", e, e.url()))?;
    //
    // debug!("Response GET {:?}", response);
    Ok(())

}

// pub fn calc_hash(name: &str) -> String {
//     let mut hash: [u8; 20] = Sha1::new().chain_update(name).finalize().into();
//     let negative = (hash[0] & 0x80) == 0x80;
//
//     // Digest is 20 bytes, so 40 hex digits plus the minus sign if necessary.
//     let mut hex = String::with_capacity(40 + negative as usize);
//     if negative {
//         hex.push('-');
//
//         // two's complement
//         let mut carry = true;
//         for b in hash.iter_mut().rev() {
//             (*b, carry) = (!*b).overflowing_add(carry as u8);
//         }
//     }
//     hex.extend(
//         hash.into_iter()
//             // extract hex digits
//             .flat_map(|x| [x >> 4, x & 0xf])
//             // skip leading zeroes
//             .skip_while(|&x| x == 0)
//             .map(|x| char::from_digit(x as u32, 16).expect("x is always valid base16")),
//     );
//     hex
// }


pub(crate) async fn encrypt(verify_token_plain: &[u8], shared_secret_plain: &[u8], assets: Arc<Assets>, connection: &mut Connection) -> Result<(), String> {
    assert_eq!(verify_token_plain, connection.verify_token);
    debug!("shared secret: {:?}", shared_secret_plain);
    let hash = compute_server_hash(assets.clone(), shared_secret_plain);
    request(connection.player.clone(), hash).await?;
    Ok(())
}

fn compute_server_hash(assets: Arc<Assets>, shared_secret: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(b""); // server ID - always empty
    hasher.update(&shared_secret);
    let key = rsa_der::public_key_to_der(&assets.key.n().to_vec(), &assets.key.e().to_vec());
    hasher.update(&key);
    hexdigest(hasher.finalize_fixed_reset().as_slice())
}

// Non-standard hex digest used by Minecraft.
fn hexdigest(bytes: &[u8]) -> String {
    let bigint = BigInt::from_signed_bytes_be(bytes);
    format!("{:x}", bigint)
}