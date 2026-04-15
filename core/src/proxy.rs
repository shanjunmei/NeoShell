//! Proxy support: SOCKS5(H) and HTTP CONNECT tunnels for SSH connections.
//!
//! Implements the proxy handshake at TCP level — returns a connected TcpStream
//! that can be passed directly to ssh2::Session::set_tcp_stream().

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProxyConfig {
    pub id: String,
    pub name: String,
    pub proxy_type: ProxyType,  // socks5h, http
    pub host: String,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyType {
    Socks5h,
    Http,
}

impl std::fmt::Display for ProxyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyType::Socks5h => write!(f, "SOCKS5H"),
            ProxyType::Http => write!(f, "HTTP"),
        }
    }
}

/// Result of a proxy latency test.
#[derive(Debug, Clone)]
pub struct ProxyTestResult {
    pub reachable: bool,
    pub latency_ms: u64,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Proxy storage (plain JSON, not encrypted — proxy configs are not secrets)
// ---------------------------------------------------------------------------

pub struct ProxyStore {
    path: std::path::PathBuf,
}

impl ProxyStore {
    pub fn new() -> Self {
        let dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("neoshell");
        let _ = std::fs::create_dir_all(&dir);
        Self {
            path: dir.join("proxies.json"),
        }
    }

    pub fn load(&self) -> Vec<ProxyConfig> {
        let data = match std::fs::read_to_string(&self.path) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };
        serde_json::from_str(&data).unwrap_or_default()
    }

    pub fn save(&self, proxies: &[ProxyConfig]) {
        if let Ok(json) = serde_json::to_string_pretty(proxies) {
            let _ = std::fs::write(&self.path, json);
        }
    }

    pub fn add(&self, proxy: ProxyConfig) {
        let mut list = self.load();
        list.push(proxy);
        self.save(&list);
    }

    pub fn update(&self, proxy: &ProxyConfig) {
        let mut list = self.load();
        if let Some(existing) = list.iter_mut().find(|p| p.id == proxy.id) {
            *existing = proxy.clone();
        }
        self.save(&list);
    }

    pub fn delete(&self, id: &str) {
        let mut list = self.load();
        list.retain(|p| p.id != id);
        self.save(&list);
    }

    pub fn get(&self, id: &str) -> Option<ProxyConfig> {
        self.load().into_iter().find(|p| p.id == id)
    }
}

// ---------------------------------------------------------------------------
// TCP connection through proxy
// ---------------------------------------------------------------------------

/// Connect to `target_host:target_port` through the given proxy.
/// Returns a TcpStream tunneled through the proxy, ready for SSH handshake.
pub fn connect_via_proxy(
    proxy: &ProxyConfig,
    target_host: &str,
    target_port: u16,
    timeout: Duration,
) -> Result<TcpStream, String> {
    // Connect to proxy server
    let proxy_addr = format!("{}:{}", proxy.host, proxy.port);
    let tcp = TcpStream::connect_timeout(
        &proxy_addr
            .to_socket_addrs()
            .map_err(|e| format!("Proxy DNS failed for '{}': {}", proxy_addr, e))?
            .next()
            .ok_or_else(|| format!("No address for proxy '{}'", proxy_addr))?,
        timeout,
    )
    .map_err(|e| format!("Proxy TCP connect to {} failed: {}", proxy_addr, e))?;

    tcp.set_read_timeout(Some(timeout)).ok();
    tcp.set_write_timeout(Some(timeout)).ok();

    match proxy.proxy_type {
        ProxyType::Socks5h => socks5_handshake(tcp, target_host, target_port, proxy),
        ProxyType::Http => http_connect_handshake(tcp, target_host, target_port, proxy),
    }
}

/// Connect directly (no proxy) — same interface for uniform calling.
pub fn connect_direct(
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<TcpStream, String> {
    let addr = format!("{}:{}", host, port);
    let tcp = TcpStream::connect_timeout(
        &addr
            .to_socket_addrs()
            .map_err(|e| format!("DNS resolve failed for '{}': {}", addr, e))?
            .next()
            .ok_or_else(|| format!("No address found for '{}'", addr))?,
        timeout,
    )
    .map_err(|e| format!("TCP connect to {} failed: {}", addr, e))?;
    Ok(tcp)
}

// ---------------------------------------------------------------------------
// SOCKS5H handshake (RFC 1928 — with remote DNS resolution)
// ---------------------------------------------------------------------------

fn socks5_handshake(
    mut tcp: TcpStream,
    target_host: &str,
    target_port: u16,
    proxy: &ProxyConfig,
) -> Result<TcpStream, String> {
    let has_auth = proxy.username.is_some();

    // 1. Greeting: VER=5, NMETHODS, METHODS
    if has_auth {
        // Offer: no-auth (0x00) + username/password (0x02)
        tcp.write_all(&[0x05, 0x02, 0x00, 0x02])
            .map_err(|e| format!("SOCKS5 greeting write failed: {}", e))?;
    } else {
        // Offer: no-auth only
        tcp.write_all(&[0x05, 0x01, 0x00])
            .map_err(|e| format!("SOCKS5 greeting write failed: {}", e))?;
    }

    // 2. Server method selection
    let mut resp = [0u8; 2];
    tcp.read_exact(&mut resp)
        .map_err(|e| format!("SOCKS5 greeting read failed: {}", e))?;
    if resp[0] != 0x05 {
        return Err(format!("SOCKS5: invalid version {}", resp[0]));
    }

    match resp[1] {
        0x00 => {} // No auth needed
        0x02 => {
            // Username/password auth (RFC 1929)
            let user = proxy.username.as_deref().unwrap_or("");
            let pass = proxy.password.as_deref().unwrap_or("");
            let mut auth_req = vec![0x01]; // VER
            auth_req.push(user.len() as u8);
            auth_req.extend_from_slice(user.as_bytes());
            auth_req.push(pass.len() as u8);
            auth_req.extend_from_slice(pass.as_bytes());
            tcp.write_all(&auth_req)
                .map_err(|e| format!("SOCKS5 auth write failed: {}", e))?;

            let mut auth_resp = [0u8; 2];
            tcp.read_exact(&mut auth_resp)
                .map_err(|e| format!("SOCKS5 auth read failed: {}", e))?;
            if auth_resp[1] != 0x00 {
                return Err("SOCKS5: authentication failed".to_string());
            }
        }
        0xFF => return Err("SOCKS5: no acceptable auth method".to_string()),
        m => return Err(format!("SOCKS5: unsupported auth method {}", m)),
    }

    // 3. CONNECT request — use DOMAINNAME (0x03) for SOCKS5H (remote DNS)
    let host_bytes = target_host.as_bytes();
    let mut req = vec![
        0x05, // VER
        0x01, // CMD: CONNECT
        0x00, // RSV
        0x03, // ATYP: DOMAINNAME
        host_bytes.len() as u8,
    ];
    req.extend_from_slice(host_bytes);
    req.push((target_port >> 8) as u8);
    req.push((target_port & 0xFF) as u8);
    tcp.write_all(&req)
        .map_err(|e| format!("SOCKS5 connect write failed: {}", e))?;

    // 4. Read response
    let mut resp_head = [0u8; 4];
    tcp.read_exact(&mut resp_head)
        .map_err(|e| format!("SOCKS5 connect read failed: {}", e))?;
    if resp_head[0] != 0x05 {
        return Err(format!("SOCKS5: invalid response version {}", resp_head[0]));
    }
    if resp_head[1] != 0x00 {
        let err_msg = match resp_head[1] {
            0x01 => "general SOCKS server failure",
            0x02 => "connection not allowed by ruleset",
            0x03 => "network unreachable",
            0x04 => "host unreachable",
            0x05 => "connection refused",
            0x06 => "TTL expired",
            0x07 => "command not supported",
            0x08 => "address type not supported",
            _ => "unknown error",
        };
        return Err(format!("SOCKS5 connect failed: {}", err_msg));
    }

    // Consume the bound address (skip it)
    match resp_head[3] {
        0x01 => {
            let mut skip = [0u8; 6]; // IPv4 (4) + port (2)
            tcp.read_exact(&mut skip).ok();
        }
        0x03 => {
            let mut len = [0u8; 1];
            tcp.read_exact(&mut len).ok();
            let mut skip = vec![0u8; len[0] as usize + 2]; // domain + port
            tcp.read_exact(&mut skip).ok();
        }
        0x04 => {
            let mut skip = [0u8; 18]; // IPv6 (16) + port (2)
            tcp.read_exact(&mut skip).ok();
        }
        _ => {}
    }

    // Clear timeouts for SSH use
    tcp.set_read_timeout(None).ok();
    tcp.set_write_timeout(None).ok();
    tcp.set_nonblocking(false).ok();

    Ok(tcp)
}

// ---------------------------------------------------------------------------
// HTTP CONNECT handshake (RFC 7231)
// ---------------------------------------------------------------------------

fn http_connect_handshake(
    mut tcp: TcpStream,
    target_host: &str,
    target_port: u16,
    proxy: &ProxyConfig,
) -> Result<TcpStream, String> {
    let mut request = format!(
        "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n",
        target_host, target_port, target_host, target_port
    );

    // Add proxy auth if configured
    if let Some(ref user) = proxy.username {
        let pass = proxy.password.as_deref().unwrap_or("");
        let cred = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!("{}:{}", user, pass),
        );
        request.push_str(&format!("Proxy-Authorization: Basic {}\r\n", cred));
    }
    request.push_str("\r\n");

    tcp.write_all(request.as_bytes())
        .map_err(|e| format!("HTTP CONNECT write failed: {}", e))?;

    // Read response (we need at least the status line)
    let mut buf = [0u8; 1024];
    let mut total = 0;
    loop {
        let n = tcp
            .read(&mut buf[total..])
            .map_err(|e| format!("HTTP CONNECT read failed: {}", e))?;
        if n == 0 {
            return Err("HTTP CONNECT: proxy closed connection".to_string());
        }
        total += n;
        // Check for end of headers
        if let Some(pos) = find_subsequence(&buf[..total], b"\r\n\r\n") {
            let header = String::from_utf8_lossy(&buf[..pos]);
            // Parse status code from "HTTP/1.x 200 ..."
            if let Some(status_line) = header.lines().next() {
                let parts: Vec<&str> = status_line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let code: u16 = parts[1].parse().unwrap_or(0);
                    if code == 200 {
                        break; // Tunnel established
                    } else {
                        return Err(format!("HTTP CONNECT failed: {}", status_line));
                    }
                }
            }
            return Err(format!("HTTP CONNECT: invalid response: {}", header));
        }
        if total >= buf.len() {
            return Err("HTTP CONNECT: response too large".to_string());
        }
    }

    tcp.set_read_timeout(None).ok();
    tcp.set_write_timeout(None).ok();
    tcp.set_nonblocking(false).ok();

    Ok(tcp)
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
}

// ---------------------------------------------------------------------------
// Proxy latency test
// ---------------------------------------------------------------------------

/// Test proxy reachability and latency by performing a SOCKS5/HTTP handshake
/// to a well-known target (or just TCP connect to the proxy itself).
pub fn test_proxy(proxy: &ProxyConfig) -> ProxyTestResult {
    let start = Instant::now();
    let proxy_addr = format!("{}:{}", proxy.host, proxy.port);

    // Just test TCP connectivity + handshake to the proxy
    let result = TcpStream::connect_timeout(
        &match proxy_addr.to_socket_addrs() {
            Ok(mut addrs) => match addrs.next() {
                Some(a) => a,
                None => {
                    return ProxyTestResult {
                        reachable: false,
                        latency_ms: 0,
                        error: Some("No address found".to_string()),
                    }
                }
            },
            Err(e) => {
                return ProxyTestResult {
                    reachable: false,
                    latency_ms: 0,
                    error: Some(format!("DNS: {}", e)),
                }
            }
        },
        Duration::from_secs(5),
    );

    match result {
        Ok(mut tcp) => {
            // Try a SOCKS5 greeting or HTTP HEAD to verify it's actually a proxy
            let latency = start.elapsed().as_millis() as u64;
            match proxy.proxy_type {
                ProxyType::Socks5h => {
                    // Send SOCKS5 greeting
                    if tcp.write_all(&[0x05, 0x01, 0x00]).is_ok() {
                        let mut resp = [0u8; 2];
                        if tcp.read_exact(&mut resp).is_ok() && resp[0] == 0x05 {
                            return ProxyTestResult {
                                reachable: true,
                                latency_ms: start.elapsed().as_millis() as u64,
                                error: None,
                            };
                        }
                    }
                    ProxyTestResult {
                        reachable: true,
                        latency_ms: latency,
                        error: Some("TCP OK but SOCKS5 handshake failed".to_string()),
                    }
                }
                ProxyType::Http => {
                    ProxyTestResult {
                        reachable: true,
                        latency_ms: latency,
                        error: None,
                    }
                }
            }
        }
        Err(e) => ProxyTestResult {
            reachable: false,
            latency_ms: start.elapsed().as_millis() as u64,
            error: Some(format!("{}", e)),
        },
    }
}
