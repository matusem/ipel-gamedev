//! Signed deploy webhook: CI posts a release notification; the host pulls and restarts compose.
//!
//! Requires `DEPLOY_WEBHOOK_PUBLIC_KEY` (base64 Ed25519 verify key, 32 bytes) and a mounted
//! Docker socket + compose directory (`DEPLOY_COMPOSE_DIR`, default `/deploy`).
//!
//! When `DEPLOY_WEBHOOK_TOKEN` is set, requests must include a matching `X-Deploy-Token` header
//! (used with a Cloudflare WAF skip rule so CI can reach this endpoint).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Deserialize;
use tracing::{error, info, warn};

static DEPLOY_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Deserialize)]
pub struct DeployRequest {
    pub image: String,
    pub tag: String,
}

pub fn is_enabled() -> bool {
    std::env::var("DEPLOY_WEBHOOK_PUBLIC_KEY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

fn public_key() -> Option<VerifyingKey> {
    let b64 = std::env::var("DEPLOY_WEBHOOK_PUBLIC_KEY").ok()?;
    let bytes = BASE64.decode(b64.trim()).ok()?;
    let arr: [u8; 32] = bytes.as_slice().try_into().ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}

fn compose_dir() -> PathBuf {
    PathBuf::from(
        std::env::var("DEPLOY_COMPOSE_DIR").unwrap_or_else(|_| "/deploy".into()),
    )
}

fn max_skew_secs() -> u64 {
    std::env::var("DEPLOY_WEBHOOK_MAX_SKEW_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300)
}

fn signed_message(timestamp: &str, body: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(timestamp.len() + 1 + body.len());
    msg.extend_from_slice(timestamp.as_bytes());
    msg.push(b'\n');
    msg.extend_from_slice(body);
    msg
}

fn deploy_token() -> Option<String> {
    std::env::var("DEPLOY_WEBHOOK_TOKEN")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn verify_deploy_token(req: &HttpRequest) -> Result<(), HttpResponse> {
    let Some(expected) = deploy_token() else {
        return Ok(());
    };

    let provided = req
        .headers()
        .get("x-deploy-token")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| HttpResponse::Unauthorized().body("missing X-Deploy-Token"))?;

    if !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        return Err(HttpResponse::Unauthorized().body("invalid X-Deploy-Token"));
    }

    Ok(())
}

fn verify_request(req: &HttpRequest, body: &[u8]) -> Result<(), HttpResponse> {
    if let Err(resp) = verify_deploy_token(req) {
        return Err(resp);
    }

    let Some(key) = public_key() else {
        return Err(HttpResponse::ServiceUnavailable().body("deploy webhook not configured"));
    };

    let timestamp = req
        .headers()
        .get("x-deploy-timestamp")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| HttpResponse::Unauthorized().body("missing X-Deploy-Timestamp"))?;

    let sig_b64 = req
        .headers()
        .get("x-deploy-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| HttpResponse::Unauthorized().body("missing X-Deploy-Signature"))?;

    let ts: u64 = timestamp
        .parse()
        .map_err(|_| HttpResponse::Unauthorized().body("invalid X-Deploy-Timestamp"))?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let skew = now.abs_diff(ts);
    if skew > max_skew_secs() {
        return Err(HttpResponse::Unauthorized().body("timestamp outside allowed window"));
    }

    let sig_bytes = BASE64
        .decode(sig_b64.trim())
        .map_err(|_| HttpResponse::Unauthorized().body("invalid X-Deploy-Signature encoding"))?;
    let sig_arr: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| HttpResponse::Unauthorized().body("invalid signature length"))?;
    let signature = Signature::from_bytes(&sig_arr);

    let msg = signed_message(timestamp, body);
    key.verify(&msg, &signature)
        .map_err(|_| HttpResponse::Unauthorized().body("signature verification failed"))?;

    Ok(())
}

fn validate_image_ref(image: &str, tag: &str) -> Result<String, HttpResponse> {
    let image = image.trim();
    let tag = tag.trim();
    if image.is_empty() || tag.is_empty() {
        return Err(HttpResponse::BadRequest().body("image and tag are required"));
    }
    if image.contains([' ', '\n', '\r', ';', '|', '&', '$', '`']) || tag.contains([' ', '\n', '\r', ';', '|', '&', '$', '`', '/'])
    {
        return Err(HttpResponse::BadRequest().body("invalid image or tag"));
    }
    Ok(format!("{image}:{tag}"))
}

fn compose_files(dir: &Path) -> Result<(PathBuf, PathBuf), HttpResponse> {
    let compose = dir.join("docker-compose.yml");
    let env_file = dir.join(".env");
    if !compose.is_file() {
        return Err(HttpResponse::ServiceUnavailable().body("docker-compose.yml not found"));
    }
    if !env_file.is_file() {
        return Err(HttpResponse::ServiceUnavailable().body(".env not found"));
    }
    Ok((compose, env_file))
}

async fn run_compose_deploy(image_ref: String, compose: PathBuf, env_file: PathBuf) {
    let dir = compose_dir();
    let script = format!(
        "set -euo pipefail; export GAMEDEV_IMAGE='{image_ref}'; \
         docker compose -f '{}' --env-file '{}' pull; \
         docker compose -f '{}' --env-file '{}' up -d --remove-orphans",
        compose.display(),
        env_file.display(),
        compose.display(),
        env_file.display(),
    );

    info!(%image_ref, dir = %dir.display(), "deploy webhook: starting compose pull/up");

    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(&script)
        .current_dir(&dir)
        .output()
        .await;

    match output {
        Ok(out) => {
            if out.status.success() {
                info!("deploy webhook: compose finished successfully");
            } else {
                error!(
                    status = ?out.status,
                    stderr = %String::from_utf8_lossy(&out.stderr),
                    stdout = %String::from_utf8_lossy(&out.stdout),
                    "deploy webhook: compose failed"
                );
            }
        }
        Err(e) => error!(error = %e, "deploy webhook: failed to spawn compose"),
    }

    DEPLOY_IN_PROGRESS.store(false, Ordering::SeqCst);
}

pub async fn handle_deploy(
    req: HttpRequest,
    body: web::Bytes,
) -> ActixResult<HttpResponse> {
    if !is_enabled() {
        return Ok(HttpResponse::ServiceUnavailable().body("deploy webhook not configured"));
    }

    if let Err(resp) = verify_request(&req, &body) {
        return Ok(resp);
    }

    let payload: DeployRequest = serde_json::from_slice(&body)
        .map_err(|_| actix_web::error::ErrorBadRequest("invalid JSON body"))?;

    let image_ref = match validate_image_ref(&payload.image, &payload.tag) {
        Ok(v) => v,
        Err(resp) => return Ok(resp),
    };

    let (compose, env_file) = match compose_files(&compose_dir()) {
        Ok(v) => v,
        Err(resp) => {
            DEPLOY_IN_PROGRESS.store(false, Ordering::SeqCst);
            return Ok(resp);
        }
    };

    if DEPLOY_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        warn!("deploy webhook: request rejected — deploy already in progress");
        return Ok(HttpResponse::Conflict().body("deploy already in progress"));
    }

    tokio::spawn(run_compose_deploy(image_ref.clone(), compose, env_file));

    Ok(HttpResponse::Accepted().json(serde_json::json!({
        "status": "accepted",
        "image": image_ref,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn verify_valid_signature() {
        let signing = SigningKey::from_bytes(&[7u8; 32]);
        let verifying = signing.verifying_key();
        unsafe {
            std::env::set_var(
                "DEPLOY_WEBHOOK_PUBLIC_KEY",
                BASE64.encode(verifying.to_bytes()),
            );
            std::env::set_var("DEPLOY_WEBHOOK_MAX_SKEW_SECS", "999999999");
        }

        let body = br#"{"image":"ghcr.io/org/app","tag":"1.0.0"}"#;
        let ts = "1700000000";
        let msg = signed_message(ts, body);
        let sig = BASE64.encode(signing.sign(&msg).to_bytes());

        let req = actix_web::test::TestRequest::post()
            .insert_header(("X-Deploy-Timestamp", ts))
            .insert_header(("X-Deploy-Signature", sig))
            .to_http_request();

        assert!(verify_request(&req, body).is_ok());
        unsafe {
            std::env::remove_var("DEPLOY_WEBHOOK_PUBLIC_KEY");
            std::env::remove_var("DEPLOY_WEBHOOK_MAX_SKEW_SECS");
        }
    }

    #[test]
    fn rejects_bad_tag() {
        assert!(validate_image_ref("ghcr.io/org/app", "1.0/bad").is_err());
    }

    #[test]
    fn verify_deploy_token_when_configured() {
        unsafe {
            std::env::set_var("DEPLOY_WEBHOOK_TOKEN", "ci-bypass-secret");
        }

        let ok = actix_web::test::TestRequest::post()
            .insert_header(("X-Deploy-Token", "ci-bypass-secret"))
            .to_http_request();
        assert!(verify_deploy_token(&ok).is_ok());

        let missing = actix_web::test::TestRequest::post().to_http_request();
        assert!(verify_deploy_token(&missing).is_err());

        let bad = actix_web::test::TestRequest::post()
            .insert_header(("X-Deploy-Token", "wrong"))
            .to_http_request();
        assert!(verify_deploy_token(&bad).is_err());

        unsafe {
            std::env::remove_var("DEPLOY_WEBHOOK_TOKEN");
        }
    }
}
