use num_traits::FromPrimitive;
use std::ffi::c_char;
use tracing::{enabled, event, Level};
use tracing_subscriber::EnvFilter;

#[derive(FromPrimitive)]
enum CppLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Critical,
    Off,
}

impl From<CppLogLevel> for tracing::Level {
    fn from(value: CppLogLevel) -> Self {
        match value {
            CppLogLevel::Trace => Level::TRACE,
            CppLogLevel::Debug => Level::DEBUG,
            CppLogLevel::Info => Level::INFO,
            CppLogLevel::Warn => Level::WARN,
            CppLogLevel::Error => Level::ERROR,
            CppLogLevel::Critical => Level::ERROR,
            CppLogLevel::Off => Level::TRACE,
        }
    }
}

#[no_mangle]
pub extern "C" fn rsn_log_init() {
    let dirs = std::env::var(EnvFilter::DEFAULT_ENV).unwrap_or(String::from(
        "rsnano_ffi=debug,rsnano_node=debug,rsnano_messages=debug,rsnano_ledger=debug,rsnano_store_lmdb=debug,rsnano_core=debug",
    ));
    let filter = EnvFilter::builder().parse_lossy(dirs);

    tracing_subscriber::fmt::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init()
}

#[no_mangle]
pub extern "C" fn rsn_log_init_test() {
    let dirs = std::env::var(EnvFilter::DEFAULT_ENV).unwrap_or(String::from("off"));
    let filter = EnvFilter::builder().parse_lossy(dirs);

    tracing_subscriber::fmt::fmt()
        .with_env_filter(filter)
        .init()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_log(log_level: u8, message: *const c_char, len: usize) {
    let message = std::mem::transmute::<*const c_char, *const u8>(message);
    let data = std::slice::from_raw_parts(message, len);
    let message = std::str::from_utf8(data).unwrap();

    let cpp_level: CppLogLevel = FromPrimitive::from_u8(log_level).unwrap();
    let level = Level::from(cpp_level);

    //TODO log tag as well
    if level == Level::TRACE {
        event!(Level::TRACE, message);
    } else if level == Level::DEBUG {
        event!(Level::DEBUG, message);
    } else if level == Level::INFO {
        event!(Level::INFO, message);
    } else if level == Level::WARN {
        event!(Level::WARN, message);
    } else if level == Level::ERROR {
        event!(Level::ERROR, message);
    }
}

#[no_mangle]
pub extern "C" fn rsn_log_min_level() -> u8 {
    let cpp_level = if enabled!(Level::TRACE) {
        CppLogLevel::Trace
    } else if enabled!(Level::DEBUG) {
        CppLogLevel::Debug
    } else if enabled!(Level::INFO) {
        CppLogLevel::Info
    } else if enabled!(Level::WARN) {
        CppLogLevel::Warn
    } else if enabled!(Level::ERROR) {
        CppLogLevel::Error
    } else {
        CppLogLevel::Off
    };

    cpp_level as u8
}
