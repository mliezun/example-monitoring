use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use minijinja::Value;
use regex::Regex;
use url::Url;

use crate::auth::{self, User};
use crate::http::{AppResponse, AppState, FormData, RequestContext};

fn parse_status_codes(raw: &str) -> Option<String> {
    let cleaned = raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(",");
    let re = Regex::new(r"^\d{3}(,\d{3})*$").ok()?;
    if cleaned.is_empty() || !re.is_match(&cleaned) {
        return None;
    }
    for code in cleaned.split(',') {
        let value: i32 = code.parse().ok()?;
        if !(100..600).contains(&value) {
            return None;
        }
    }
    Some(cleaned)
}

fn validate_site_form(
    name: &str,
    url_raw: &str,
    interval: &str,
    codes: &str,
    retries: &str,
) -> (String, HashMap<String, String>) {
    let mut errors: HashMap<String, String> = HashMap::new();
    if name.trim().is_empty() {
        errors.insert("name".into(), "Name is required.".into());
    }
    let parsed = Url::parse(url_raw.trim());
    match parsed {
        Ok(parsed) if parsed.scheme() == "https" && parsed.host_str().is_some() => {
            let re = Regex::new(r"(?i)^https://[^\s/$.?#].[^\s]*$").unwrap();
            if !re.is_match(url_raw.trim()) {
                errors.insert("url".into(), "Enter a valid HTTPS URL.".into());
            }
        }
        _ => {
            errors.insert("url".into(), "URL must use HTTPS.".into());
        }
    }

    match interval.parse::<i64>() {
        Ok(seconds) if seconds >= 30 => {}
        Ok(_) => {
            errors.insert(
                "poll_interval_seconds".into(),
                "Minimum interval is 30 seconds.".into(),
            );
        }
        Err(_) => {
            errors.insert(
                "poll_interval_seconds".into(),
                "Interval must be a number.".into(),
            );
        }
    }

    if parse_status_codes(codes).is_none() {
        errors.insert(
            "ok_status_codes".into(),
            "Use comma-separated HTTP status codes (e.g. 200,204).".into(),
        );
    }

    match retries.parse::<i64>() {
        Ok(value) if value >= 1 => {}
        Ok(_) => {
            errors.insert(
                "max_retries".into(),
                "At least one attempt is required.".into(),
            );
        }
        Err(_) => {
            errors.insert("max_retries".into(), "Retries must be a number.".into());
        }
    }

    let message = errors.values().next().cloned().unwrap_or_default();
    let values = HashMap::from([
        ("name".into(), name.to_string()),
        ("url".into(), url_raw.to_string()),
        ("poll_interval_seconds".into(), interval.to_string()),
        ("ok_status_codes".into(), codes.to_string()),
        ("max_retries".into(), retries.to_string()),
    ]);
    (message, values)
}

fn require_user(state: &AppState, ctx: &RequestContext) -> Result<User, AppResponse> {
    auth::current_user(&state.db, ctx)
        .ok_or_else(|| AppResponse::redirect(state, ctx, "/login"))
}

fn load_history(state: &AppState, site_id: &str) -> Vec<Value> {
    state
        .db
        .fetchall("list_poll_results.sql", &[&site_id, &100_i64])
        .unwrap_or_default()
        .into_iter()
        .map(Value::from)
        .collect()
}

fn user_template(user: &User) -> Value {
    Value::from(user.template_map())
}

pub async fn list_sites(State(state): State<AppState>, headers: HeaderMap) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    let user = match require_user(&state, &ctx) {
        Ok(user) => user,
        Err(response) => return response,
    };
    let org_id = user.get_i64("org_id").unwrap();
    let sites: Vec<Value> = state
        .db
        .fetchall("list_sites_by_org.sql", &[&org_id])
        .unwrap_or_default()
        .into_iter()
        .map(Value::from)
        .collect();

    let mut vars = HashMap::new();
    vars.insert("title".into(), Value::from("Monitored sites"));
    vars.insert("user".into(), user_template(&user));
    vars.insert("sites".into(), Value::from(sites));
    AppResponse::html(&state, &ctx, "sites/list.html", true, vars)
}

pub async fn new_site_form(State(state): State<AppState>, headers: HeaderMap) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    let user = match require_user(&state, &ctx) {
        Ok(user) => user,
        Err(response) => return response,
    };
    let mut values: HashMap<String, Value> = HashMap::new();
    values.insert("name".into(), Value::from(""));
    values.insert("url".into(), Value::from("https://"));
    values.insert("poll_interval_seconds".into(), Value::from("60"));
    values.insert("ok_status_codes".into(), Value::from("200"));
    values.insert("max_retries".into(), Value::from("3"));

    let mut vars = HashMap::new();
    vars.insert("title".into(), Value::from("Add site"));
    vars.insert("user".into(), user_template(&user));
    vars.insert("error".into(), Value::from(""));
    vars.insert("values".into(), Value::from(values));
    AppResponse::html(&state, &ctx, "sites/new.html", true, vars)
}

pub async fn create_site(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    let user = match require_user(&state, &ctx) {
        Ok(user) => user,
        Err(response) => return response,
    };
    let form = FormData {
        values: crate::http::parse_form(&body).values,
    };
    if !crate::http::validate_csrf(&ctx, &form) {
        return AppResponse::forbidden(&state, &ctx, "Invalid CSRF token");
    }

    let (message, values) = validate_site_form(
        &form.get("name"),
        &form.get("url"),
        &form.get("poll_interval_seconds"),
        &form.get("ok_status_codes"),
        &form.get("max_retries"),
    );

    if !message.is_empty() {
        let mut value_map = HashMap::new();
        for (key, value) in values {
            value_map.insert(key, Value::from(value));
        }
        let mut vars = HashMap::new();
        vars.insert("title".into(), Value::from("Add site"));
        vars.insert("user".into(), user_template(&user));
        vars.insert("error".into(), Value::from(message));
        vars.insert("values".into(), Value::from(value_map));
        return AppResponse::html(&state, &ctx, "sites/new.html", true, vars);
    }

    let org_id = user.get_i64("org_id").unwrap();
    let name = values["name"].trim().to_string();
    let url = values["url"].trim().to_string();
    let interval: i64 = values["poll_interval_seconds"].parse().unwrap();
    let codes = parse_status_codes(&values["ok_status_codes"]).unwrap();
    let retries: i64 = values["max_retries"].parse().unwrap();

    let site = state
        .db
        .query_one(
            "insert_site.sql",
            &[&org_id, &name, &url, &interval, &codes, &retries],
        )
        .ok()
        .flatten();
    let Some(site) = site else {
        return AppResponse::not_found(&state, &ctx);
    };
    let site_id: i64 = site.get("id").and_then(|v| v.as_i64()).unwrap_or_default();
    AppResponse::redirect(&state, &ctx, &format!("/sites/{site_id}"))
}

pub async fn site_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(site_id): Path<String>,
) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    let user = match require_user(&state, &ctx) {
        Ok(user) => user,
        Err(response) => return response,
    };
    let org_id = user.get_i64("org_id").unwrap();
    let Some(row) = state
        .db
        .fetchone("get_site_by_id.sql", &[&site_id, &org_id])
        .ok()
        .flatten()
    else {
        return AppResponse::text(&state, &ctx, axum::http::StatusCode::NOT_FOUND, "Site not found");
    };
    let site_map = row;
    let site = Value::from(site_map.clone());
    let history = load_history(&state, &site_id);

    let mut vars = HashMap::new();
    vars.insert(
        "title".into(),
        site_map
            .get("name")
            .cloned()
            .unwrap_or(Value::from("Site")),
    );
    vars.insert("user".into(), user_template(&user));
    vars.insert("site".into(), site);
    vars.insert("history".into(), Value::from(history));
    AppResponse::html(&state, &ctx, "sites/detail.html", true, vars)
}

pub async fn site_history_partial(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(site_id): Path<String>,
) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    let user = match require_user(&state, &ctx) {
        Ok(user) => user,
        Err(response) => return response,
    };
    let org_id = user.get_i64("org_id").unwrap();
    if state
        .db
        .fetchone("get_site_by_id.sql", &[&site_id, &org_id])
        .ok()
        .flatten()
        .is_none()
    {
        return AppResponse::text(&state, &ctx, axum::http::StatusCode::NOT_FOUND, "Site not found");
    }
    let history = load_history(&state, &site_id);
    let mut vars = HashMap::new();
    vars.insert("history".into(), Value::from(history));
    AppResponse::html(&state, &ctx, "sites/_history.html", true, vars)
}

pub async fn delete_site(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(site_id): Path<String>,
    body: String,
) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    let user = match require_user(&state, &ctx) {
        Ok(user) => user,
        Err(response) => return response,
    };
    let form = FormData {
        values: crate::http::parse_form(&body).values,
    };
    if !crate::http::validate_csrf(&ctx, &form) {
        return AppResponse::forbidden(&state, &ctx, "Invalid CSRF token");
    }
    let org_id = user.get_i64("org_id").unwrap();
    let _ = state
        .db
        .execute("delete_site.sql", &[&site_id, &org_id]);
    AppResponse::redirect(&state, &ctx, "/sites")
}
