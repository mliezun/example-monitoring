mod auth;
mod config;
mod controllers;
mod db;
mod http;

use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use minijinja::Environment;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::db::Database;
use crate::http::{AppResponse, AppState, RequestContext};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    let config = Config::from_env();
    let db = Arc::new(Database::open(&config).expect("open database"));
    let mut templates = Environment::new();
    templates.set_loader(minijinja::path_loader(&config.templates_dir()));

    let state = AppState {
        config: config.clone(),
        db,
        templates: Arc::new(templates),
    };

    let app = Router::new()
        .route("/", get(controllers::landing::index))
        .route("/login", get(controllers::auth_controller::login_form).post(controllers::auth_controller::login_submit))
        .route(
            "/register",
            get(controllers::auth_controller::register_form).post(controllers::auth_controller::register_submit),
        )
        .route("/logout", post(controllers::auth_controller::logout))
        .route("/sites", get(controllers::sites::list_sites))
        .route(
            "/sites/new",
            get(controllers::sites::new_site_form).post(controllers::sites::create_site),
        )
        .route("/sites/:site_id/history", get(controllers::sites::site_history_partial))
        .route("/sites/:site_id", get(controllers::sites::site_detail))
        .route("/sites/:site_id/delete", post(controllers::sites::delete_site))
        .route(
            "/settings",
            get(controllers::settings::settings_form).post(controllers::settings::settings_submit),
        )
        .route("/health", get(health))
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .with_state(state.clone());

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port))
        .await
        .expect("bind listener");
    tracing::info!(
        "example-monitoring listening on http://{}:{}",
        config.host,
        config.port
    );
    axum::serve(listener, app).await.expect("serve");
}

async fn health(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: HeaderMap,
) -> AppResponse {
    let ctx = RequestContext::from_headers(&headers, &state.config.secret_key);
    AppResponse::text(&state, &ctx, StatusCode::OK, "ok")
}
