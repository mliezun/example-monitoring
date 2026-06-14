use std::collections::HashMap;

use axum::extract::State;
use axum::http::HeaderMap;
use minijinja::Value;

use crate::auth;
use crate::http::{AppResponse, AppState, RequestContext};

pub async fn index(State(state): State<AppState>, headers: HeaderMap) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    if auth::current_user(&state.db, &ctx).is_some() {
        return AppResponse::redirect(&state, &ctx, "/sites");
    }
    let mut vars = HashMap::new();
    vars.insert("title".into(), Value::from("example-monitoring"));
    AppResponse::html(&state, &ctx, "landing/index.html", true, vars)
}
