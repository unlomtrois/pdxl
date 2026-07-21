//! Minimal leveled stderr logging. The VS Code extension surfaces the server's
//! stderr in the "pdxl (server)" output channel, so this is the field-test
//! visibility channel. No timestamps or targets — the channel adds its own.

use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub enum Level {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
}

static LEVEL: OnceLock<Level> = OnceLock::new();

/// Sets the process log level once (from `--log-level`). Later calls ignored.
pub fn init(level: &str) {
    let l = match level {
        "debug" => Level::Debug,
        "warn" => Level::Warn,
        "error" => Level::Error,
        _ => Level::Info,
    };
    let _ = LEVEL.set(l);
}

pub fn enabled(level: Level) -> bool {
    level <= *LEVEL.get_or_init(|| Level::Info)
}

macro_rules! log_at {
    ($lvl:expr, $tag:expr, $($arg:tt)*) => {
        if $crate::log::enabled($lvl) {
            eprintln!(concat!("[", $tag, "] {}"), format!($($arg)*));
        }
    };
}
macro_rules! log_error { ($($arg:tt)*) => { log_at!($crate::log::Level::Error, "error", $($arg)*) } }
macro_rules! log_warn  { ($($arg:tt)*) => { log_at!($crate::log::Level::Warn,  "warn",  $($arg)*) } }
macro_rules! log_info  { ($($arg:tt)*) => { log_at!($crate::log::Level::Info,  "info",  $($arg)*) } }
macro_rules! log_debug { ($($arg:tt)*) => { log_at!($crate::log::Level::Debug, "debug", $($arg)*) } }
