use std::collections::HashMap;

use axum::extract::State;
use axum::http::HeaderMap;
use minijinja::Value;

use crate::auth;
use crate::http::{AppResponse, AppState, FormData, RequestContext};

pub async fn login_form(State(state): State<AppState>, headers: HeaderMap) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    let mut vars = HashMap::new();
    vars.insert("title".into(), Value::from("Log in"));
    vars.insert("error".into(), Value::from(""));
    AppResponse::html(&state, &ctx, "auth/login.html", true, vars)
}

pub async fn login_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    let form = FormData {
        values: crate::http::parse_form(&body).values,
    };
    if !crate::http::validate_csrf(&ctx, &form) {
        return AppResponse::forbidden(&state, &ctx, "Invalid CSRF token");
    }

    let username = form.get("username");
    let username = username.trim();
    let password = form.get("password");
    let invalid = || {
        let mut vars = HashMap::new();
        vars.insert("title".into(), Value::from("Log in"));
        vars.insert("error".into(), Value::from("Invalid username or password."));
        AppResponse::html(&state, &ctx, "auth/login.html", true, vars)
    };

    let Some(row) = state
        .db
        .fetchone("get_user_by_username.sql", &[&username])
        .ok()
        .flatten()
    else {
        return invalid();
    };

    let password_hash = row
        .get("password_hash")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let user_id = row.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
    if !auth::verify_password(&password, password_hash) {
        return invalid();
    }

    AppResponse::redirect_with_session(&state, &ctx, "/sites", user_id)
}

pub async fn register_form(State(state): State<AppState>, headers: HeaderMap) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    let mut vars = HashMap::new();
    vars.insert("title".into(), Value::from("Register"));
    vars.insert("error".into(), Value::from(""));
    AppResponse::html(&state, &ctx, "auth/register.html", true, vars)
}

pub async fn register_submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    let form = FormData {
        values: crate::http::parse_form(&body).values,
    };
    if !crate::http::validate_csrf(&ctx, &form) {
        return AppResponse::forbidden(&state, &ctx, "Invalid CSRF token");
    }

    let org_name = form.get("org_name").trim().to_string();
    let username = form.get("username").trim().to_string();
    let password = form.get("password");
    let confirm = form.get("password_confirm");

    let error = if org_name.is_empty() || username.is_empty() || password.is_empty() {
        "All fields are required."
    } else if username.len() < 3 {
        "Username must be at least 3 characters."
    } else if password.len() < 8 {
        "Password must be at least 8 characters."
    } else if password != confirm {
        "Passwords do not match."
    } else if state
        .db
        .fetchone("username_exists.sql", &[&username])
        .ok()
        .flatten()
        .is_some()
    {
        "Username is already taken."
    } else {
        ""
    };

    if !error.is_empty() {
        let mut vars = HashMap::new();
        vars.insert("title".into(), Value::from("Register"));
        vars.insert("error".into(), Value::from(error));
        return AppResponse::html(&state, &ctx, "auth/register.html", true, vars);
    }

    let password_hash = auth::hash_password(&password);
    let user_id = match state.db.transaction(|db, conn| {
        let org = db
            .query_one_in_tx(conn, "create_org.sql", &[&org_name])?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        let org_id = org.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
        let user = db
            .query_one_in_tx(conn, "create_user.sql", &[&org_id, &username, &password_hash])?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        Ok(user.get("id").and_then(|v| v.as_i64()).unwrap_or_default())
    }) {
        Ok(id) => id,
        Err(_) => {
            let mut vars = HashMap::new();
            vars.insert("title".into(), Value::from("Register"));
            vars.insert("error".into(), Value::from("Could not create account."));
            return AppResponse::html(&state, &ctx, "auth/register.html", true, vars);
        }
    };

    AppResponse::redirect_with_session(&state, &ctx, "/sites", user_id)
}

pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    let form = FormData {
        values: crate::http::parse_form(&body).values,
    };
    if !crate::http::validate_csrf(&ctx, &form) {
        return AppResponse::forbidden(&state, &ctx, "Invalid CSRF token");
    }
    if auth::current_user(&state.db, &ctx).is_none() {
        return AppResponse::redirect(&state, &ctx, "/login");
    }
    AppResponse::clear_session_redirect(&state, &ctx, "/")
}
