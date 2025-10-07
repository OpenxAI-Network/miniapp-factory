use ethsign::SecretKey;
use rand::{Rng, rng};
use std::fs::{read, write};

use super::env::datadir;

pub fn get_signer() -> SecretKey {
    let path = datadir().join("secret.key");
    let key = match read(&path) {
        Ok(secret) => secret
            .try_into()
            .inspect_err(|e| {
                log::error!(
                    "Private key {path} in incorrect format: {e:?}", // Might not be a good idea to log a potential private key
                    path = path.display(),
                );
            })
            .ok()
            .unwrap_or_else(generate_private_key),
        Err(e) => {
            log::warn!(
                "Could not read private key {path}: {e}",
                path = path.display()
            );

            generate_private_key()
        }
    };

    SecretKey::from_raw(&key).unwrap_or_else(|e| {
        panic!("Could not convert private key into SecretKey: {}", e);
    })
}

fn generate_private_key() -> [u8; 32] {
    log::info!("Generating new secret key");
    let priv_key = random_bytes();

    let path = datadir().join("secret.key");
    if let Err(e) = write(&path, priv_key) {
        log::error!(
            "Could not save private key {path}: {e}",
            path = path.display()
        );
    }

    priv_key
}

fn random_bytes() -> [u8; 32] {
    let mut secret = [0u8; 32];
    rng().fill(&mut secret);
    secret
}
