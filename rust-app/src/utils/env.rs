use std::path::{Path, PathBuf};

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

pub fn projectsdir() -> PathBuf {
    env_var("PROJECTSDIR")
        .map(|d| Path::new(&d).to_path_buf())
        .unwrap_or(Path::new(&datadir()).join("projects"))
}

pub fn usersdir() -> PathBuf {
    env_var("USERSDIR")
        .map(|d| Path::new(&d).to_path_buf())
        .unwrap_or(Path::new(&datadir()).join("users"))
}

pub fn model() -> String {
    env_var("MODEL").unwrap_or("gpt-oss:20b".to_string())
}

pub fn git() -> String {
    env_var("GIT").unwrap_or("".to_string())
}

pub fn aider() -> String {
    env_var("AIDER").unwrap_or("".to_string())
}
