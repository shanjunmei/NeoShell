#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod i18n;
mod proxy;
mod app;
mod crypto;
mod ssh;
mod sshconfig;
mod storage;
mod terminal;
mod ui;
pub mod updater;

/// Entry point called by the launcher via dlopen.
/// Returns: 0 = normal exit, 42 = restart for update
#[no_mangle]
pub extern "C" fn neoshell_run() -> i32 {
    env_logger::init();
    match app::run() {
        Ok(()) => 0,
        Err(_) => 1,
    }
}

/// Return the current version string.
#[no_mangle]
pub extern "C" fn neoshell_version() -> *const u8 {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr()
}
