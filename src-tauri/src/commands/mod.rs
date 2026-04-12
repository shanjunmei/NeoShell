use tauri::State;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};

use crate::storage::{ConnectionStore, ConnectionConfig, ConnectionInfo};
use crate::ssh::SshManager;

#[tauri::command]
pub fn cmd_is_master_password_set(store: State<'_, ConnectionStore>) -> Result<bool, String> {
    Ok(store.vault_exists())
}

#[tauri::command]
pub fn cmd_set_master_password(
    password: String,
    store: State<'_, ConnectionStore>,
) -> Result<(), String> {
    store.set_master_password(&password)
}

#[tauri::command]
pub fn cmd_verify_master_password(
    password: String,
    store: State<'_, ConnectionStore>,
) -> Result<bool, String> {
    store.verify_master_password(&password)
}

#[tauri::command]
pub fn cmd_unlock_vault(
    password: String,
    store: State<'_, ConnectionStore>,
) -> Result<bool, String> {
    store.unlock(&password)
}

#[tauri::command]
pub fn cmd_save_connection(
    config: ConnectionConfig,
    store: State<'_, ConnectionStore>,
) -> Result<String, String> {
    store.save_connection(config)
}

#[tauri::command]
pub fn cmd_get_connections(
    store: State<'_, ConnectionStore>,
) -> Result<Vec<ConnectionInfo>, String> {
    store.get_connections()
}

#[tauri::command]
pub fn cmd_get_connection(
    id: String,
    store: State<'_, ConnectionStore>,
) -> Result<ConnectionConfig, String> {
    store.get_connection(&id)
}

#[tauri::command]
pub fn cmd_delete_connection(
    id: String,
    store: State<'_, ConnectionStore>,
) -> Result<(), String> {
    store.delete_connection(&id)
}

#[tauri::command]
pub fn cmd_update_connection(
    config: ConnectionConfig,
    store: State<'_, ConnectionStore>,
) -> Result<(), String> {
    store.update_connection(config)
}

#[tauri::command]
pub fn cmd_connect_ssh(
    connection_id: String,
    app_handle: tauri::AppHandle,
    store: State<'_, ConnectionStore>,
    ssh_manager: State<'_, SshManager>,
) -> Result<String, String> {
    // Load full connection config with decrypted secrets
    let config = store.get_connection(&connection_id)?;

    ssh_manager.connect(
        app_handle,
        connection_id,
        &config.host,
        config.port,
        &config.username,
        &config.auth_type,
        config.password.as_deref(),
        config.private_key.as_deref(),
        config.passphrase.as_deref(),
    )
}

#[tauri::command]
pub fn cmd_disconnect_ssh(
    session_id: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    ssh_manager.disconnect(&session_id)
}

#[tauri::command]
pub fn cmd_ssh_write(
    session_id: String,
    data: String,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    // data is base64-encoded from frontend
    let bytes = BASE64.decode(&data)
        .map_err(|e| format!("Failed to decode base64 data: {}", e))?;
    ssh_manager.write(&session_id, &bytes)
}

#[tauri::command]
pub fn cmd_ssh_resize(
    session_id: String,
    cols: u32,
    rows: u32,
    ssh_manager: State<'_, SshManager>,
) -> Result<(), String> {
    ssh_manager.resize(&session_id, cols, rows)
}
