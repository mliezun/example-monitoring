import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import Database from "better-sqlite3";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const QUERIES_DIR = path.resolve(__dirname, "../db/queries");

const queryCache = new Map();

export function databasePath() {
  return process.env.DATABASE_PATH || "data/monitoring.db";
}

export function loadQuery(name) {
  if (!queryCache.has(name)) {
    const filePath = path.join(QUERIES_DIR, name);
    queryCache.set(name, fs.readFileSync(filePath, "utf8").trim());
  }
  return queryCache.get(name);
}

export function openDatabase(dbPath = databasePath()) {
  fs.mkdirSync(path.dirname(path.resolve(dbPath)), { recursive: true });
  const db = new Database(dbPath, {
    timeout: 5000,
    verbose: process.env.SQLITE_VERBOSE ? console.log : null,
  });
  db.pragma("foreign_keys = ON");
  db.pragma("journal_mode = WAL");
  db.pragma("busy_timeout = 5000");
  db.pragma("synchronous = NORMAL");
  return db;
}

export function prepare(db, queryName) {
  return db.prepare(loadQuery(queryName));
}

export function withTransaction(db, fn) {
  const run = db.transaction(fn);
  return run();
}
