use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::config::Config;

pub struct Database {
    conn: Mutex<Connection>,
    queries: Mutex<HashMap<String, String>>,
    queries_dir: std::path::PathBuf,
}

impl Database {
    pub fn open(config: &Config) -> rusqlite::Result<Self> {
        if let Some(parent) = config.database_path.parent() {
            fs::create_dir_all(parent).ok();
        }

        let conn = Connection::open(&config.database_path)?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA synchronous = NORMAL;",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
            queries: Mutex::new(HashMap::new()),
            queries_dir: config.queries_dir(),
        })
    }

    fn load_query(&self, name: &str) -> rusqlite::Result<String> {
        let mut cache = self.queries.lock().unwrap();
        if let Some(sql) = cache.get(name) {
            return Ok(sql.clone());
        }

        let path = self.queries_dir.join(name);
        let sql = fs::read_to_string(&path).map_err(|err| {
            rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Missing query file {name}: {err}"),
            )))
        })?;
        let sql = sql.trim().to_string();
        cache.insert(name.to_string(), sql.clone());
        Ok(sql)
    }

    fn map_row(row: &rusqlite::Row<'_>) -> HashMap<String, minijinja::Value> {
        row_to_map(row)
    }

    fn fetchone_on(
        &self,
        conn: &Connection,
        query_name: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> rusqlite::Result<Option<HashMap<String, minijinja::Value>>> {
        let sql = self.load_query(query_name)?;
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params)?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::map_row(&row)))
        } else {
            Ok(None)
        }
    }

    fn fetchall_on(
        &self,
        conn: &Connection,
        query_name: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> rusqlite::Result<Vec<HashMap<String, minijinja::Value>>> {
        let sql = self.load_query(query_name)?;
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(params)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            out.push(Self::map_row(&row));
        }
        Ok(out)
    }

    fn execute_on(
        &self,
        conn: &Connection,
        query_name: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> rusqlite::Result<usize> {
        let sql = self.load_query(query_name)?;
        conn.execute(&sql, params)
    }

    pub fn fetchone(
        &self,
        query_name: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> rusqlite::Result<Option<HashMap<String, minijinja::Value>>> {
        let conn = self.conn.lock().unwrap();
        self.fetchone_on(&conn, query_name, params)
    }

    pub fn fetchall(
        &self,
        query_name: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> rusqlite::Result<Vec<HashMap<String, minijinja::Value>>> {
        let conn = self.conn.lock().unwrap();
        self.fetchall_on(&conn, query_name, params)
    }

    pub fn execute(
        &self,
        query_name: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> rusqlite::Result<usize> {
        let conn = self.conn.lock().unwrap();
        self.execute_on(&conn, query_name, params)
    }

    pub fn query_one(
        &self,
        query_name: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> rusqlite::Result<Option<HashMap<String, minijinja::Value>>> {
        self.fetchone(query_name, params)
    }

    pub fn transaction<F, T>(&self, f: F) -> rusqlite::Result<T>
    where
        F: FnOnce(&Self, &Connection) -> rusqlite::Result<T>,
    {
        let conn = self.conn.lock().unwrap();
        conn.execute("BEGIN IMMEDIATE", [])?;
        match f(self, &conn) {
            Ok(value) => {
                conn.execute("COMMIT", [])?;
                Ok(value)
            }
            Err(err) => {
                conn.execute("ROLLBACK", [])?;
                Err(err)
            }
        }
    }

    pub fn fetchone_in_tx(
        &self,
        conn: &Connection,
        query_name: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> rusqlite::Result<Option<HashMap<String, minijinja::Value>>> {
        self.fetchone_on(conn, query_name, params)
    }

    pub fn query_one_in_tx(
        &self,
        conn: &Connection,
        query_name: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> rusqlite::Result<Option<HashMap<String, minijinja::Value>>> {
        self.fetchone_on(conn, query_name, params)
    }

    pub fn execute_in_tx(
        &self,
        conn: &Connection,
        query_name: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> rusqlite::Result<usize> {
        self.execute_on(conn, query_name, params)
    }
}

pub fn row_to_map(row: &rusqlite::Row<'_>) -> HashMap<String, minijinja::Value> {
    let mut map = HashMap::new();
    for idx in 0..row.as_ref().column_count() {
        let name = row.as_ref().column_name(idx).unwrap_or("").to_string();
        let value: minijinja::Value = match row.get_ref(idx).unwrap() {
            rusqlite::types::ValueRef::Null => minijinja::Value::from(()),
            rusqlite::types::ValueRef::Integer(v) => minijinja::Value::from(v),
            rusqlite::types::ValueRef::Real(v) => minijinja::Value::from(v),
            rusqlite::types::ValueRef::Text(v) => {
                minijinja::Value::from(std::str::from_utf8(v).unwrap_or("").to_string())
            }
            rusqlite::types::ValueRef::Blob(v) => {
                minijinja::Value::from(String::from_utf8_lossy(v).to_string())
            }
        };
        map.insert(name, value);
    }
    map
}
