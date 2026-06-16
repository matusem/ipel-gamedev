//! Google OAuth 2.0 authorization-code flow.

use actix_web::cookie::{Cookie, SameSite};
use actix_web::{HttpRequest, HttpResponse, web};
use serde::Deserialize;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::auth_sessions;
use crate::db;
use crate::user_engagement;

const SESSION_TOKEN_KEY: &str = "upjs_gdd_session_token";
const USER_ID_KEY: &str = "upjs_gdd_user_id";
const OAUTH_STATE_COOKIE: &str = "oauth_state";

#[derive(Debug, Clone, Deserialize)]
pub struct GoogleUserInfo {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
}

pub fn is_configured() -> bool {
    std::env::var("GOOGLE_CLIENT_ID")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
        && std::env::var("GOOGLE_CLIENT_SECRET")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
}

fn client_id() -> Option<String> {
    std::env::var("GOOGLE_CLIENT_ID").ok().filter(|v| !v.trim().is_empty())
}

fn client_secret() -> Option<String> {
    std::env::var("GOOGLE_CLIENT_SECRET")
        .ok()
        .filter(|v| !v.trim().is_empty())
}

/// Public-facing origin for OAuth redirect (must match Google Console).
pub fn redirect_base() -> String {
    if let Ok(host) = std::env::var("OAUTH_REDIRECT_HOST") {
        let trimmed = host.trim().trim_end_matches('/');
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    if port == 8081 {
        "http://127.0.0.1:8080".to_string()
    } else {
        format!("http://127.0.0.1:{port}")
    }
}

pub fn redirect_uri() -> String {
    format!("{}/auth/google/callback", redirect_base())
}

pub fn authorization_url(state: &str) -> Result<String, String> {
    let client_id = client_id().ok_or_else(|| "Google OAuth not configured".to_string())?;
    let redirect = redirect_uri();
    let params = [
        ("client_id", client_id.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("response_type", "code"),
        ("scope", "openid email profile"),
        ("state", state),
        ("access_type", "online"),
        ("prompt", "select_account"),
    ];
    let qs = serde_urlencoded::to_string(&params).map_err(|e| e.to_string())?;
    Ok(format!("https://accounts.google.com/o/oauth2/v2/auth?{qs}"))
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

pub async fn exchange_code(code: &str) -> Result<GoogleUserInfo, String> {
    let client_id = client_id().ok_or_else(|| "Google OAuth not configured".to_string())?;
    let client_secret = client_secret().ok_or_else(|| "Google OAuth not configured".to_string())?;
    let redirect = redirect_uri();

    let client = reqwest::Client::new();
    let token_resp = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| format!("token request failed: {e}"))?;

    if !token_resp.status().is_success() {
        let body = token_resp.text().await.unwrap_or_default();
        return Err(format!("token exchange failed: {body}"));
    }

    let token: TokenResponse = token_resp
        .json()
        .await
        .map_err(|e| format!("token parse failed: {e}"))?;

    let user_resp = client
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(&token.access_token)
        .send()
        .await
        .map_err(|e| format!("userinfo request failed: {e}"))?;

    if !user_resp.status().is_success() {
        let body = user_resp.text().await.unwrap_or_default();
        return Err(format!("userinfo failed: {body}"));
    }

    user_resp
        .json::<GoogleUserInfo>()
        .await
        .map_err(|e| format!("userinfo parse failed: {e}"))
}

fn session_success_html(session_token: &str, user_id: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>Signing in…</title>
</head>
<body>
  <p>Signing you in…</p>
  <script>
    try {{
      localStorage.setItem("{SESSION_TOKEN_KEY}", {token_json});
      localStorage.setItem("{USER_ID_KEY}", {user_json});
    }} catch (e) {{}}
    window.location.replace("/");
  </script>
</body>
</html>"#,
        token_json = serde_json::to_string(session_token).unwrap_or_default(),
        user_json = serde_json::to_string(user_id).unwrap_or_default(),
    )
}

fn oauth_error_html(message: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8" /><title>Sign-in failed</title></head>
<body>
  <p>Sign-in failed: {message}</p>
  <p><a href="/">Back to login</a></p>
</body>
</html>"#
    )
}

pub async fn google_start(req: HttpRequest) -> HttpResponse {
    if !is_configured() {
        return HttpResponse::NotFound().body("Google OAuth is not configured");
    }

    let state = Uuid::new_v4().to_string();
    let url = match authorization_url(&state) {
        Ok(u) => u,
        Err(e) => return HttpResponse::InternalServerError().body(e),
    };

    let mut cookie = Cookie::build(OAUTH_STATE_COOKIE, state)
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(actix_web::cookie::time::Duration::minutes(5))
        .finish();

    if req.connection_info().scheme() == "https" {
        cookie.set_secure(true);
    }

    HttpResponse::Found()
        .cookie(cookie)
        .append_header(("Location", url))
        .finish()
}

#[derive(Deserialize)]
pub struct GoogleCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub async fn google_callback(
    req: HttpRequest,
    pool: web::Data<SqlitePool>,
    query: web::Query<GoogleCallbackQuery>,
) -> HttpResponse {
    if let Some(err) = &query.error {
        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(oauth_error_html(err));
    }

    let Some(code) = query.code.as_deref() else {
        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(oauth_error_html("missing authorization code"));
    };

    let Some(state) = query.state.as_deref() else {
        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(oauth_error_html("missing state"));
    };

    let cookie_state = req.cookie(OAUTH_STATE_COOKIE).map(|c| c.value().to_string());
    if cookie_state.as_deref() != Some(state) {
        return HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body(oauth_error_html("invalid OAuth state"));
    }

    let google_user = match exchange_code(code).await {
        Ok(u) => u,
        Err(e) => {
            return HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(oauth_error_html(&e));
        }
    };

    let display_name = google_user
        .name
        .clone()
        .or_else(|| google_user.email.clone())
        .unwrap_or_else(|| format!("user_{}", &google_user.sub[..8.min(google_user.sub.len())]));

    let (user_id, _name, _created, is_new) = match db::find_or_create_google_user(
        pool.get_ref(),
        &google_user.sub,
        &display_name,
    )
    .await
    {
        Ok(row) => row,
        Err(e) => {
            return HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(oauth_error_html(&format!("db: {e}")));
        }
    };

    if is_new {
        let _ = user_engagement::welcome_notification(pool.get_ref(), user_id).await;
    }

    let (session_token, _expires) = match auth_sessions::create_session(pool.get_ref(), user_id).await
    {
        Ok(s) => s,
        Err(e) => {
            return HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(oauth_error_html(&format!("session: {e}")));
        }
    };

    let clear_cookie = Cookie::build(OAUTH_STATE_COOKIE, "")
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(actix_web::cookie::time::Duration::seconds(0))
        .finish();

    HttpResponse::Ok()
        .cookie(clear_cookie)
        .content_type("text/html; charset=utf-8")
        .body(session_success_html(&session_token, &user_id.to_string()))
}
