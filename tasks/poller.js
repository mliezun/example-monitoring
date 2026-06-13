import { openDatabase, prepare, withTransaction } from "./db.js";
import { sendNotification } from "./notifications.js";

const POLL_TICK_MS = Number(process.env.POLL_TICK_MS || 5000);
const REQUEST_TIMEOUT_MS = Number(process.env.REQUEST_TIMEOUT_MS || 10000);

function parseOkCodes(raw) {
  return new Set(
    raw
      .split(",")
      .map((part) => part.trim())
      .filter(Boolean)
      .map((code) => Number(code)),
  );
}

async function probeUrl(url) {
  const started = Date.now();
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  try {
    const response = await fetch(url, {
      method: "GET",
      redirect: "follow",
      signal: controller.signal,
      headers: { "User-Agent": "example-monitoring/0.1" },
    });
    return {
      ok: true,
      statusCode: response.status,
      responseTimeMs: Date.now() - started,
      errorMessage: null,
    };
  } catch (error) {
    return {
      ok: false,
      statusCode: null,
      responseTimeMs: Date.now() - started,
      errorMessage: error instanceof Error ? error.message : String(error),
    };
  } finally {
    clearTimeout(timer);
  }
}

async function pollSite(site) {
  const okCodes = parseOkCodes(site.ok_status_codes);
  let attemptsUsed = 0;
  let lastAttempt = null;

  for (let attempt = 1; attempt <= site.max_retries; attempt += 1) {
    attemptsUsed = attempt;
    lastAttempt = await probeUrl(site.url);
    if (lastAttempt.ok && okCodes.has(lastAttempt.statusCode)) {
      return {
        status: "up",
        httpStatusCode: lastAttempt.statusCode,
        responseTimeMs: lastAttempt.responseTimeMs,
        attemptsUsed,
        errorMessage: null,
      };
    }
  }

  return {
    status: "down",
    httpStatusCode: lastAttempt?.statusCode ?? null,
    responseTimeMs: lastAttempt?.responseTimeMs ?? null,
    attemptsUsed,
    errorMessage: lastAttempt?.errorMessage || `No OK response in ${site.max_retries} attempts`,
  };
}

function statusChanged(previousStatus, nextStatus) {
  if (previousStatus === nextStatus) {
    return false;
  }
  if (previousStatus === "unknown") {
    return nextStatus === "up" || nextStatus === "down";
  }
  return true;
}

async function processDueSites(db) {
  const dueSites = prepare(db, "get_sites_due_for_poll.sql").all();
  for (const site of dueSites) {
    const result = await pollSite(site);
    const previousStatus = site.current_status;
    const nextStatus = result.status;

    withTransaction(db, () => {
      prepare(db, "insert_poll_result.sql").run(
        site.id,
        result.status,
        result.httpStatusCode,
        result.responseTimeMs,
        result.attemptsUsed,
        result.errorMessage,
      );
      prepare(db, "update_site_after_poll.sql").run(
        nextStatus,
        site.poll_interval_seconds,
        site.id,
      );
    });

    if (statusChanged(previousStatus, nextStatus)) {
      try {
        await sendNotification(site, previousStatus, nextStatus);
        console.log(`[notify] ${site.name}: ${previousStatus} -> ${nextStatus}`);
      } catch (error) {
        console.error(`[notify] failed for ${site.name}:`, error);
      }
    }

    console.log(
      `[poll] ${site.name} ${nextStatus} (${result.httpStatusCode ?? "n/a"}, ${result.attemptsUsed} attempts)`,
    );
  }
}

async function waitForDatabase(dbPath) {
  for (let attempt = 0; attempt < 30; attempt += 1) {
    try {
      const db = openDatabase(dbPath);
      db.prepare("SELECT 1").get();
      return db;
    } catch (error) {
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }
  throw new Error(`Database not ready at ${dbPath}`);
}

async function main() {
  const dbPath = process.env.DATABASE_PATH || "data/monitoring.db";
  const db = await waitForDatabase(dbPath);
  console.log(`example-monitoring poller connected to ${dbPath}`);

  const tick = async () => {
    try {
      await processDueSites(db);
    } catch (error) {
      console.error("[poll] cycle failed:", error);
    }
  };

  await tick();
  setInterval(tick, POLL_TICK_MS);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});

export { pollSite, processDueSites, statusChanged };
