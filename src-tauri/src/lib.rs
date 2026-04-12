mod crypto;
mod ssh;
mod storage;
mod commands;

use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .manage(ssh::SshManager::new())
        .manage(storage::ConnectionStore::new())
        .invoke_handler(tauri::generate_handler![
            // crypto/auth
            cmd_set_master_password,
            cmd_verify_master_password,
            cmd_is_master_password_set,
            cmd_unlock_vault,
            // connections
            cmd_save_connection,
            cmd_get_connections,
            cmd_get_connection,
            cmd_delete_connection,
            cmd_update_connection,
            // ssh
            cmd_connect_ssh,
            cmd_disconnect_ssh,
            cmd_ssh_write,
            cmd_ssh_resize,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
