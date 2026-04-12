use std::collections::HashMap;
use std::io::Read as IoRead;
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use parking_lot::RwLock;
use serde::Serialize;
use ssh2::Session;
use tauri::Emitter;

pub enum SshCommand {
    Write(Vec<u8>),
    Resize(u32, u32),
    Disconnect,
}

#[allow(dead_code)]
pub struct SshSession {
    pub session_id: String,
    pub connection_id: String,
    pub writer: tokio::sync::mpsc::Sender<SshCommand>,
}

pub struct SshManager {
    sessions: RwLock<HashMap<String, SshSession>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SshDataEvent {
    session_id: String,
    data: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SshCloseEvent {
    session_id: String,
}

impl SshManager {
    pub fn new() -> Self {
        SshManager {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn connect(
        &self,
        app_handle: tauri::AppHandle,
        connection_id: String,
        host: &str,
        port: u16,
        username: &str,
        auth_type: &str,
        password: Option<&str>,
        private_key: Option<&str>,
        passphrase: Option<&str>,
    ) -> Result<String, String> {
        let session_id = uuid::Uuid::new_v4().to_string();

        // 1. TCP connect
        let addr = format!("{}:{}", host, port);
        let tcp = TcpStream::connect(&addr)
            .map_err(|e| format!("TCP connect to {} failed: {}", addr, e))?;

        // Set a read timeout for the TCP stream so the reader thread
        // can periodically check for disconnect signals
        tcp.set_read_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| format!("Failed to set read timeout: {}", e))?;

        // 2. SSH session + handshake
        let mut session = Session::new()
            .map_err(|e| format!("Failed to create SSH session: {}", e))?;
        session.set_tcp_stream(tcp);
        session.handshake()
            .map_err(|e| format!("SSH handshake failed: {}", e))?;

        // 3. Authentication
        match auth_type {
            "password" => {
                let pw = password.ok_or("Password required for password auth")?;
                session.userauth_password(username, pw)
                    .map_err(|e| format!("Password auth failed: {}", e))?;
            }
            "key" => {
                let key_data = private_key.ok_or("Private key required for key auth")?;
                // Write key to a temporary file for ssh2
                let tmp_dir = std::env::temp_dir();
                let key_path = tmp_dir.join(format!("neoshell_key_{}", session_id));
                std::fs::write(&key_path, key_data)
                    .map_err(|e| format!("Failed to write temp key: {}", e))?;

                let result = session.userauth_pubkey_file(
                    username,
                    None,
                    &key_path,
                    passphrase,
                );

                // Clean up temp key immediately
                let _ = std::fs::remove_file(&key_path);

                result.map_err(|e| format!("Key auth failed: {}", e))?;
            }
            other => {
                return Err(format!("Unsupported auth type: {}", other));
            }
        }

        if !session.authenticated() {
            return Err("Authentication failed".to_string());
        }

        // 4. Open channel and request PTY
        let mut channel = session.channel_session()
            .map_err(|e| format!("Failed to open channel: {}", e))?;

        channel.request_pty("xterm", None, Some((80, 24, 0, 0)))
            .map_err(|e| format!("PTY request failed: {}", e))?;

        // 5. Request shell
        channel.shell()
            .map_err(|e| format!("Shell request failed: {}", e))?;

        // Wrap session in Arc for sharing between threads
        let session = Arc::new(parking_lot::Mutex::new(session));
        let channel = Arc::new(parking_lot::Mutex::new(channel));

        // 6. Create command channel
        let (tx, mut rx) = tokio::sync::mpsc::channel::<SshCommand>(256);

        let sid = session_id.clone();

        // Store the session
        self.sessions.write().insert(
            session_id.clone(),
            SshSession {
                session_id: session_id.clone(),
                connection_id,
                writer: tx,
            },
        );

        // 7. Spawn reader thread
        let reader_channel = Arc::clone(&channel);
        let reader_app = app_handle.clone();
        let reader_sid = sid.clone();

        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                let read_result = {
                    let mut ch = reader_channel.lock();
                    if ch.eof() {
                        break;
                    }
                    ch.read(&mut buf)
                };

                match read_result {
                    Ok(0) => {
                        // EOF
                        break;
                    }
                    Ok(n) => {
                        let encoded = BASE64.encode(&buf[..n]);
                        let _ = reader_app.emit(
                            "ssh-data",
                            SshDataEvent {
                                session_id: reader_sid.clone(),
                                data: encoded,
                            },
                        );
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                    {
                        // Read timeout - just continue the loop
                        continue;
                    }
                    Err(_) => {
                        break;
                    }
                }
            }

            // Emit close event
            let _ = reader_app.emit(
                "ssh-close",
                SshCloseEvent {
                    session_id: reader_sid,
                },
            );
        });

        // 8. Spawn writer thread
        let writer_channel = Arc::clone(&channel);
        let _writer_session = Arc::clone(&session);

        std::thread::spawn(move || {
            use std::io::Write as IoWrite;
            loop {
                match rx.blocking_recv() {
                    Some(SshCommand::Write(data)) => {
                        let mut ch = writer_channel.lock();
                        if ch.write_all(&data).is_err() {
                            break;
                        }
                        let _ = ch.flush();
                    }
                    Some(SshCommand::Resize(cols, rows)) => {
                        let mut ch = writer_channel.lock();
                        let _ = ch.request_pty_size(cols, rows, None, None);
                    }
                    Some(SshCommand::Disconnect) | None => {
                        let mut ch = writer_channel.lock();
                        let _ = ch.send_eof();
                        let _ = ch.close();
                        let _ = ch.wait_close();
                        break;
                    }
                }
            }
        });

        Ok(session_id)
    }

    pub fn write(&self, session_id: &str, data: &[u8]) -> Result<(), String> {
        let sessions = self.sessions.read();
        let session = sessions.get(session_id)
            .ok_or_else(|| format!("Session '{}' not found", session_id))?;

        session.writer.try_send(SshCommand::Write(data.to_vec()))
            .map_err(|e| format!("Failed to send write command: {}", e))
    }

    pub fn resize(&self, session_id: &str, cols: u32, rows: u32) -> Result<(), String> {
        let sessions = self.sessions.read();
        let session = sessions.get(session_id)
            .ok_or_else(|| format!("Session '{}' not found", session_id))?;

        session.writer.try_send(SshCommand::Resize(cols, rows))
            .map_err(|e| format!("Failed to send resize command: {}", e))
    }

    pub fn disconnect(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.write();
        let session = sessions.remove(session_id)
            .ok_or_else(|| format!("Session '{}' not found", session_id))?;

        let _ = session.writer.try_send(SshCommand::Disconnect);
        Ok(())
    }
}
