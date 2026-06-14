use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Config {
    pub app_root: PathBuf,
    pub database_path: PathBuf,
    pub secret_key: Vec<u8>,
    pub host: String,
    pub port: u16,
    pub cookie_secure: bool,
    pub cors_allow_origin: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        let app_root = env::var("APP_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into()))
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."))
            });

        Self {
            app_root: app_root.clone(),
            database_path: env::var("DATABASE_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| app_root.join("data/monitoring.db")),
            secret_key: env::var("SECRET_KEY")
                .unwrap_or_else(|_| "dev-secret-change-me".into())
                .into_bytes(),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(8000),
            cookie_secure: env::var("COOKIE_SECURE").unwrap_or_else(|_| "0".into()) == "1",
            cors_allow_origin: env::var("CORS_ALLOW_ORIGIN").ok(),
        }
    }

    pub fn queries_dir(&self) -> PathBuf {
        self.app_root.join("db/queries")
    }

    pub fn templates_dir(&self) -> PathBuf {
        self.app_root.join("templates")
    }
}
