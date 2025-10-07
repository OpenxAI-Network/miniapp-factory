use std::time::SystemTime;

pub fn get_time() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Invalid system time (duration from unix epoch).")
        .as_secs()
}
