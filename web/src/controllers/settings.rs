use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use minijinja::Value;

use crate::auth::{self, User};
use crate::http::{AppResponse, AppState, FormData, RequestContext};

fn require_user(state: &AppState, ctx: &RequestContext) -> Result<User, AppResponse> {
    auth::current_user(&state.db, ctx)
        .ok_or_else(|| AppResponse::redirect(state, ctx, "/login"))
}

fn user_template(user: &User) -> Value {
    Value::from(user.template_map())
}

pub async fn settings_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    let user = match require_user(&state, &ctx) {
        Ok(user) => user,
        Err(response) => return response,
    };

    let mut values: HashMap<String, Value> = HashMap::new();
    values.insert(
        "notification_provider".into(),
        Value::from(user.get_str("notification_provider").unwrap_or("")),
    );
    values.insert(
        "webhook_url".into(),
        Value::from(user.get_str("webhook_url").unwrap_or("")),
    );

    let mut vars = HashMap::new();
    vars.insert("title".into(), Value::from("Notification settings"));
    vars.insert("user".into(), user_template(&user));
    vars.insert("error".into(), Value::from(""));
    vars.insert(
        "saved".into(),
        Value::from(query.get("saved").map(String::as_str) == Some("1")),
    );
    vars.insert("values".into(), Value::from(values));
    AppResponse::html(&state, &ctx, "settings/index.html", true, vars)
}

pub async fn settings_submit(
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

    let provider = form.get("notification_provider").trim().to_lowercase();
    let mut webhook_url = form.get("webhook_url").trim().to_string();
    let valid = ["", "slack", "discord"];

    let error = if !valid.contains(&provider.as_str()) {
        "Choose Slack or Discord."
    } else if !provider.is_empty() && !webhook_url.starts_with("https://") {
        "Webhook URL must use HTTPS."
    } else {
        ""
    };

    let (provider_param, webhook_param): (Option<&str>, Option<&str>) = if provider.is_empty() {
        webhook_url.clear();
        (None, None)
    } else {
        (Some(provider.as_str()), Some(webhook_url.as_str()))
    };

    let mut values: HashMap<String, Value> = HashMap::new();
    values.insert(
        "notification_provider".into(),
        Value::from(provider_param.unwrap_or("")),
    );
    values.insert(
        "webhook_url".into(),
        Value::from(webhook_param.unwrap_or("")),
    );

    if !error.is_empty() {
        let mut vars = HashMap::new();
        vars.insert("title".into(), Value::from("Notification settings"));
        vars.insert("user".into(), user_template(&user));
        vars.insert("error".into(), Value::from(error));
        vars.insert("saved".into(), Value::from(false));
        vars.insert("values".into(), Value::from(values));
        return AppResponse::html(&state, &ctx, "settings/index.html", true, vars);
    }

    let org_id = user.get_i64("org_id").unwrap();
    let _ = state.db.execute(
        "update_org_notifications.sql",
        &[&provider_param, &webhook_param, &org_id],
    );
    AppResponse::redirect(&state, &ctx, "/settings?saved=1")
}
