use std::path::{Path, PathBuf};

use alloy::primitives::Address;

fn env_var(id: &str) -> Option<String> {
    std::env::var(id)
        .inspect_err(|e| {
            log::warn!("Could not read env var {id}: {e}");
        })
        .ok()
}

pub fn hostname() -> String {
    env_var("HOSTNAME").unwrap_or(String::from("0.0.0.0"))
}

pub fn port() -> String {
    env_var("PORT").unwrap_or(String::from("54428"))
}

pub fn datadir() -> PathBuf {
    env_var("DATADIR")
        .map(|d| Path::new(&d).to_path_buf())
        .unwrap_or(Path::new("/var/lib/miniapp-factory").to_path_buf())
}

pub fn ghtoken() -> String {
    env_var("GH_TOKEN").expect("No GH_TOKEN supplied.")
}

pub fn gh() -> String {
    env_var("GH").unwrap_or("".to_string())
}

pub fn database() -> String {
    env_var("DATABASE").unwrap_or("postgres:openxai-indexer?host=/run/postgresql".to_string())
}

pub fn nftminterkey() -> String {
    env_var("NFTMINTERKEY").expect("No NFTMINTERKEY provided.")
}

pub fn httprpc() -> String {
    env_var("HTTPRPC").unwrap_or("https://base-rpc.publicnode.com".to_string())
}

pub fn wsrpc() -> String {
    env_var("WSRPC").unwrap_or("wss://base-rpc.publicnode.com".to_string())
}

pub fn deposit() -> Address {
    Address::parse_checksummed(
        env_var("DEPOSIT").unwrap_or("0xC96d00a5e1d03b719ADD5A855ba84d05561D9897".to_string()),
        None,
    )
    .unwrap_or_else(|e| panic!("Invalid DEPOSIT provided: {e}"))
}

pub fn openx() -> Address {
    Address::parse_checksummed(
        env_var("OPENX").unwrap_or("0xA66B448f97CBf58D12f00711C02bAC2d9EAC6f7f".to_string()),
        None,
    )
    .unwrap_or_else(|e| panic!("Invalid USDC provided: {e}"))
}

pub fn nft() -> Address {
    Address::parse_checksummed(
        env_var("NFT").unwrap_or("0xBdf5f85BE2d92465d1a1865Bd1aF4B84a352b27C".to_string()),
        None,
    )
    .unwrap_or_else(|e| panic!("Invalid NFT provided: {e}"))
}

pub fn hyperstackapikey() -> String {
    env_var("HYPERSTACKAPIKEY").expect("No HYPERSTACKAPIKEY provided.")
}
