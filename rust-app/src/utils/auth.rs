use hex::ToHex;
use xnode_manager_sdk::utils::Session;

use crate::utils::{error::ResponseError, time::get_time_u64};

use super::{keccak::hash_message, wallet::get_signer};

pub async fn get_session(url: &str, domain: &str) -> Result<Session, ResponseError> {
    let signer = get_signer();

    let addr: String = signer.public().address().encode_hex();
    let user = format!("eth:{addr}");
    let timestamp = get_time_u64();
    let message = format!("Xnode Auth authenticate {domain} at {timestamp}");
    let message_bytes = hash_message(&message);
    let signature = match signer.sign(&message_bytes) {
        Ok(sig) => {
            let bytes: Vec<u8> = sig.r.into_iter().chain(sig.s).chain([sig.v]).collect();
            let hex: String = bytes.encode_hex();

            format!("0x{hex}")
        }
        Err(e) => {
            log::error!("Eth Sign error: {e}");
            return Err(ResponseError::new("Couldn't sign authentication message."));
        }
    };

    xnode_manager_sdk::auth::login(xnode_manager_sdk::auth::LoginInput {
        base_url: url.to_string(),
        user: xnode_manager_sdk::auth::User::with_signature(user, signature, timestamp.to_string()),
    })
    .await
    .map_err(|e| {
        log::error!("Xnode Manager SDK error: {e:?}");

        ResponseError::new("Couldn't sign authentication message.")
    })
}
