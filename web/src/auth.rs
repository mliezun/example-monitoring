use std::collections::HashMap;

use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::db::Database;
use crate::http::RequestContext;

const PBKDF2_ITERATIONS: u32 = 600_000;

pub struct User {
    pub fields: HashMap<String, minijinja::Value>,
}

impl User {
    pub fn from_map(fields: HashMap<String, minijinja::Value>) -> Self {
        Self { fields }
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.fields.get(key).and_then(|v| v.as_i64())
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.fields.get(key).and_then(|v| v.as_str())
    }

    pub fn template_map(&self) -> HashMap<String, minijinja::Value> {
        self.fields.clone()
    }
}

pub fn hash_password(password: &str) -> String {
    let mut salt = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut salt);
    let mut digest = [0u8; 32];
    pbkdf2_hmac::<Sha256>(
        password.as_bytes(),
        &salt,
        PBKDF2_ITERATIONS,
        &mut digest,
    );
    format!(
        "pbkdf2_sha256${}${}${}",
        PBKDF2_ITERATIONS,
        hex::encode(salt),
        hex::encode(digest)
    )
}

pub fn verify_password(password: &str, stored: &str) -> bool {
    let parts: Vec<&str> = stored.split('$').collect();
    if parts.len() != 4 || parts[0] != "pbkdf2_sha256" {
        return false;
    }
    let Ok(iterations) = parts[1].parse::<u32>() else {
        return false;
    };
    let Ok(salt) = hex::decode(parts[2]) else {
        return false;
    };
    let Ok(expected) = hex::decode(parts[3]) else {
        return false;
    };
    let mut actual = vec![0u8; expected.len()];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, iterations, &mut actual);
    actual.ct_eq(&expected).into()
}

pub fn current_user(database: &Database, ctx: &RequestContext) -> Option<User> {
    let user_id = ctx.session.get("user_id")?.as_i64()?;
    let row = database.fetchone("get_user_by_id.sql", &[&user_id]).ok()??;
    Some(User::from_map(row))
}
