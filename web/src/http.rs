use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, Mac};
use minijinja::{Environment, Value};
use rand::RngCore;
use serde_json::{json, Value as JsonValue};
use sha2::Sha256;

use crate::config::Config;
use crate::db::Database;

pub const SESSION_COOKIE: &str = "em_session";
pub const CSRF_COOKIE: &str = "em_csrf";
const SESSION_TTL_SECONDS: i64 = 60 * 60 * 24 * 14;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: Arc<Database>,
    pub templates: Arc<Environment<'static>>,
}

pub struct RequestContext {
    pub session: HashMap<String, JsonValue>,
    pub csrf_token: String,
    pub cookies: HashMap<String, String>,
    pub origin: Option<String>,
}

impl RequestContext {
    pub fn from_headers(headers: &HeaderMap, secret_key: &[u8]) -> Self {
        let cookies = parse_cookies(headers);
        let session = decode_session(cookies.get(SESSION_COOKIE).map(String::as_str), secret_key);
        let csrf_token = cookies
            .get(CSRF_COOKIE)
            .cloned()
            .unwrap_or_else(new_csrf_token);
        let origin = headers
            .get(header::ORIGIN)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);

        Self {
            session,
            csrf_token,
            cookies,
            origin,
        }
    }

    pub fn user_id(&self) -> Option<i64> {
        self.session.get("user_id").and_then(|v| v.as_i64())
    }
}

pub struct FormData {
    pub values: HashMap<String, String>,
}

impl FormData {
    pub fn get(&self, key: &str) -> String {
        self.values.get(key).cloned().unwrap_or_default()
    }
}

pub fn parse_form(body: &str) -> FormData {
    let mut values = HashMap::new();
    for (key, value) in url::form_urlencoded::parse(body.as_bytes()) {
        values.insert(key.into_owned(), value.into_owned());
    }
    FormData { values }
}

pub fn parse_query(query: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        values.insert(key.into_owned(), value.into_owned());
    }
    values
}

pub fn parse_cookies(headers: &HeaderMap) -> HashMap<String, String> {
    let mut cookies = HashMap::new();
    if let Some(raw) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        for part in raw.split(';') {
            let part = part.trim();
            if let Some((key, value)) = part.split_once('=') {
                cookies.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }
    cookies
}

pub fn new_csrf_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64_url_encode(&bytes)
}

fn base64_url_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        write!(&mut out, "{}", TABLE[((n >> 18) & 63) as usize] as char).ok();
        write!(&mut out, "{}", TABLE[((n >> 12) & 63) as usize] as char).ok();
        if chunk.len() > 1 {
            write!(&mut out, "{}", TABLE[((n >> 6) & 63) as usize] as char).ok();
        }
        if chunk.len() > 2 {
            write!(&mut out, "{}", TABLE[(n & 63) as usize] as char).ok();
        }
    }
    out
}

fn sign_payload(payload: &str, secret_key: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret_key).expect("hmac key");
    mac.update(payload.as_bytes());
    let digest = hex::encode(mac.finalize().into_bytes());
    format!("{payload}.{digest}")
}

fn verify_payload(signed: &str, secret_key: &[u8]) -> Option<String> {
    let (payload, digest) = signed.rsplit_once('.')?;
    let mut mac = HmacSha256::new_from_slice(secret_key).ok()?;
    mac.update(payload.as_bytes());
    mac.verify_slice(&hex::decode(digest).ok()?).ok()?;
    Some(payload.to_string())
}

pub fn encode_session(data: &HashMap<String, JsonValue>, secret_key: &[u8]) -> String {
    let exp = unix_now() + SESSION_TTL_SECONDS;
    let payload = json!({ "data": data, "exp": exp }).to_string();
    sign_payload(&payload, secret_key)
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

pub fn decode_session(raw: Option<&str>, secret_key: &[u8]) -> HashMap<String, JsonValue> {
    let Some(raw) = raw else {
        return HashMap::new();
    };
    let Some(payload) = verify_payload(raw, secret_key) else {
        return HashMap::new();
    };
    let Ok(parsed) = serde_json::from_str::<JsonValue>(&payload) else {
        return HashMap::new();
    };
    let Some(exp) = parsed.get("exp").and_then(|v| v.as_i64()) else {
        return HashMap::new();
    };
    if exp < unix_now() {
        return HashMap::new();
    }
    let Some(data) = parsed.get("data").and_then(|v| v.as_object()) else {
        return HashMap::new();
    };
    data.iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub fn validate_csrf(ctx: &RequestContext, form: &FormData) -> bool {
    let submitted = form.get("_csrf");
    !submitted.is_empty() && submitted == ctx.csrf_token
}

fn set_cookie(name: &str, value: &str, max_age: i64, secure: bool) -> HeaderValue {
    let mut cookie = format!(
        "{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}"
    );
    if secure {
        cookie.push_str("; Secure");
    }
    HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static(""))
}

fn delete_cookie(name: &str, secure: bool) -> HeaderValue {
    set_cookie(name, "", 0, secure)
}

pub fn security_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("X-Frame-Options", HeaderValue::from_static("DENY"));
    headers.insert(
        "Referrer-Policy",
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        "Content-Security-Policy",
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' https://unpkg.com https://cdn.tailwindcss.com 'unsafe-inline'; style-src 'self' https://cdn.tailwindcss.com 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; base-uri 'self'; form-action 'self'; frame-ancestors 'none'",
        ),
    );
    headers.insert(
        "Permissions-Policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );
    headers
}

pub struct AppResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Body,
}

impl AppResponse {
    pub fn html(
        state: &AppState,
        ctx: &RequestContext,
        template: &str,
        attach_csrf: bool,
        vars: HashMap<String, Value>,
    ) -> Self {
        let mut context_map = vars;
        context_map.insert(
            "csrf_token".into(),
            Value::from(ctx.csrf_token.clone()),
        );
        let rendered = state
            .templates
            .get_template(template)
            .and_then(|t| t.render(context_map))
            .unwrap_or_else(|err| format!("Template error: {err}"));

        let mut response = Self {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Body::from(rendered),
        };
        response
            .headers
            .insert(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"));
        if attach_csrf {
            response.push_csrf_cookie(ctx, &state.config);
        }
        response.apply_security_and_cors(&state.config, ctx);
        response
    }

    pub fn redirect(state: &AppState, ctx: &RequestContext, location: &str) -> Self {
        let mut response = Self {
            status: StatusCode::FOUND,
            headers: HeaderMap::new(),
            body: Body::empty(),
        };
        response.headers.insert(
            header::LOCATION,
            HeaderValue::from_str(location).unwrap_or_else(|_| HeaderValue::from_static("/")),
        );
        response.apply_security_and_cors(&state.config, ctx);
        response
    }

    pub fn redirect_with_session(
        state: &AppState,
        ctx: &RequestContext,
        location: &str,
        user_id: i64,
    ) -> Self {
        let mut session = HashMap::new();
        session.insert("user_id".to_string(), json!(user_id));
        let mut response = Self::redirect(state, ctx, location);
        response.headers.append(
            header::SET_COOKIE,
            set_cookie(
                SESSION_COOKIE,
                &encode_session(&session, &state.config.secret_key),
                SESSION_TTL_SECONDS,
                state.config.cookie_secure,
            ),
        );
        response
    }

    pub fn clear_session_redirect(state: &AppState, ctx: &RequestContext, location: &str) -> Self {
        let mut response = Self::redirect(state, ctx, location);
        response.headers.append(
            header::SET_COOKIE,
            delete_cookie(SESSION_COOKIE, state.config.cookie_secure),
        );
        response
    }

    pub fn text(state: &AppState, ctx: &RequestContext, status: StatusCode, body: &str) -> Self {
        let mut response = Self {
            status,
            headers: HeaderMap::new(),
            body: Body::from(body.to_string()),
        };
        response.headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        response.apply_security_and_cors(&state.config, ctx);
        response
    }

    pub fn forbidden(state: &AppState, ctx: &RequestContext, body: &str) -> Self {
        Self::text(state, ctx, StatusCode::FORBIDDEN, body)
    }

    pub fn not_found(state: &AppState, ctx: &RequestContext) -> Self {
        Self::text(state, ctx, StatusCode::NOT_FOUND, "Not Found")
    }

    fn push_csrf_cookie(&mut self, ctx: &RequestContext, config: &Config) {
        self.headers.append(
            header::SET_COOKIE,
            set_cookie(
                CSRF_COOKIE,
                &ctx.csrf_token,
                SESSION_TTL_SECONDS,
                config.cookie_secure,
            ),
        );
    }

    fn apply_security_and_cors(&mut self, config: &Config, ctx: &RequestContext) {
        self.headers.extend(security_headers());
        if let Some(allowed) = &config.cors_allow_origin {
            if ctx.origin.as_deref() == Some(allowed.as_str()) {
                self.headers.insert(
                    header::ACCESS_CONTROL_ALLOW_ORIGIN,
                    HeaderValue::from_str(allowed).unwrap(),
                );
                self.headers.insert(
                    header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                    HeaderValue::from_static("true"),
                );
                self.headers.insert(header::VARY, HeaderValue::from_static("Origin"));
            }
        }
    }
}

impl IntoResponse for AppResponse {
    fn into_response(self) -> Response {
        (self.status, self.headers, self.body).into_response()
    }
}

pub fn json_to_value(value: &JsonValue) -> Value {
    match value {
        JsonValue::Null => Value::from(()),
        JsonValue::Bool(v) => Value::from(*v),
        JsonValue::Number(v) => Value::from(v.as_i64().unwrap_or(0)),
        JsonValue::String(v) => Value::from(v.clone()),
        JsonValue::Array(v) => Value::from(
            v.iter()
                .map(json_to_value)
                .collect::<Vec<_>>(),
        ),
        JsonValue::Object(v) => Value::from(
            v.iter()
                .map(|(k, val)| (k.clone(), json_to_value(val)))
                .collect::<HashMap<String, Value>>(),
        ),
    }
}

pub fn ctx(template: minijinja::Value) -> minijinja::Value {
    template
}
