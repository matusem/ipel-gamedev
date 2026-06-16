//! Browser loopback login (RFC 8252-style redirect to 127.0.0.1).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use rand::RngCore;
use tiny_http::{Header, Response, Server};

use crate::reporter::{self, SpinnerFinish};

pub struct WebLoginResult {
    pub token: String,
    pub user_id: String,
    pub expires_at: i64,
}

const LOGIN_TIMEOUT: Duration = Duration::from_secs(180);

pub fn login_via_browser(platform_base: &str) -> Result<WebLoginResult> {
    let state = generate_state();
    let server = Server::http("127.0.0.1:0")
        .map_err(|e| anyhow::anyhow!("failed to bind loopback listener on 127.0.0.1: {e}"))?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|a| a.port())
        .context("loopback server has no IP address")?;

    let base = platform_base.trim_end_matches('/');
    let auth_url = format!(
        "{base}/cli-auth?port={port}&state={}",
        urlencoding::encode(&state)
    );

    reporter::status("login", "opening browser for authentication...");
    if webbrowser::open(&auth_url).is_err() {
        reporter::warn("browser", "could not open browser automatically");
        reporter::hint(&format!("Open this URL manually:\n  {auth_url}"));
    } else {
        reporter::hint(&format!("Waiting for browser callback on 127.0.0.1:{port} (timeout 180s)"));
    }

    let spinner = reporter::spinner("Waiting for login in browser...");
    let deadline = Instant::now() + LOGIN_TIMEOUT;

    loop {
        if Instant::now() >= deadline {
            spinner.finish_fail("login timed out after 180s");
            bail!("browser login timed out");
        }
        let request = match server.recv_timeout(Duration::from_millis(400)) {
            Ok(Some(req)) => req,
            Ok(None) => continue,
            Err(e) => {
                spinner.finish_fail("loopback server error");
                return Err(e.into());
            }
        };

        let path = request.url().to_string();
        if !path.starts_with("/callback") {
            let _ = request.respond(html_response(
                404,
                "Not found",
                "<p>Expected /callback from the lobby CLI auth page.</p>",
            ));
            continue;
        }

        let params = parse_query(path.strip_prefix("/callback").unwrap_or(""));
        let got_state = params.get("state").cloned().unwrap_or_default();
        if got_state != state {
            let _ = request.respond(html_response(
                400,
                "Invalid state",
                "<p>CSRF state mismatch. Close this tab and run <code>gamedev login</code> again.</p>",
            ));
            spinner.finish_fail("invalid state in callback");
            bail!("browser login rejected: state mismatch");
        }

        let Some(token) = params.get("token").cloned() else {
            let _ = request.respond(html_response(
                400,
                "Missing token",
                "<p>No token in callback. Try again from the CLI.</p>",
            ));
            spinner.finish_fail("missing token");
            bail!("browser login callback missing token");
        };

        let expires_at = params
            .get("expires")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64 + 7 * 24 * 60 * 60)
                    .unwrap_or(0)
            });
        let user_id = params
            .get("user")
            .cloned()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "cli-user".to_string());

        let _ = request.respond(html_response(
            200,
            "Login successful",
            "<h1>Login successful</h1><p>You can close this tab and return to your terminal.</p>",
        ));

        spinner.finish_ok("received credentials from browser");
        return Ok(WebLoginResult {
            token,
            user_id,
            expires_at,
        });
    }
}

fn generate_state() -> String {
    let mut rng = rand::thread_rng();
    let mut bytes = [0u8; 16];
    rng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_query(qs: &str) -> HashMap<String, String> {
    let qs = qs.trim_start_matches('?');
    urlencoding::decode(qs)
        .map(|s| s.into_owned())
        .unwrap_or_else(|_| qs.to_string())
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let k = parts.next()?;
            let v = parts.next().unwrap_or("");
            Some((k.to_string(), v.to_string()))
        })
        .collect()
}

fn html_response(status: u16, title: &str, body: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let html = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{title}</title></head><body>{body}</body></html>"
    );
    Response::from_string(html)
        .with_status_code(status)
        .with_header(Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap())
}
