//! Cloudflare Worker faucet API (Rust / workers-rs).
//!
//! A thin authenticated edge API backed by D1. It enforces an HTTP Basic Auth
//! bot gate, email OTP via Resend, sessioned access, and a per-email/address/IP
//! cooldown, then delegates the actual transaction to the signer service. It
//! never holds the seed and never builds transactions.

use faucet_core::{
    DripResponse, FaucetBalanceResponse, Network, Pool, SignerSendRequest, SignerSendResponse,
    validate_destination,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use worker::wasm_bindgen::JsValue;
use worker::*;

// ---------------------------------------------------------------------------
// Entry point and routing
// ---------------------------------------------------------------------------

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Routes are mounted under /api so the Worker can share one origin with the
    // Pages-hosted frontend (a Cloudflare route maps `<domain>/api/*` to this
    // Worker). Same origin makes the Basic Auth gate and session cookie work.
    Router::new()
        .get("/api/health", |_req, _ctx| Response::ok("ok"))
        .post_async("/api/auth/send-otp", handle_send_otp)
        .post_async("/api/auth/verify-otp", handle_verify_otp)
        .post_async("/api/auth/logout", handle_logout)
        .get_async("/api/faucet/status", handle_status)
        .get_async("/api/faucet/stats", handle_stats)
        .get_async("/api/faucet/balance", handle_faucet_balance)
        .get_async("/api/faucet/services", handle_services)
        .post_async("/api/internal/balance", handle_ingest_balance)
        .post_async("/api/faucet/drip", handle_drip)
        .run(req, env)
        .await
}

/// Cron-triggered chain-liveness heartbeat.
///
/// The Cloudflare Cron Trigger (see `wrangler.toml` `[triggers]`) invokes this
/// on a schedule (every 5 minutes). It asks the signer to send a tiny Orchard
/// amount to the faucet's OWN unified address (a self-send), producing a real
/// testnet transaction every tick. This exercises the full signer pipeline
/// (sync -> build -> prove -> broadcast) end to end, so a long-running
/// deployment continuously proves the whole faucet still works; if the pipeline
/// rots (sync stalls, prover params missing, zaino unreachable), these runs
/// fail and surface in `wrangler tail` / the Worker logs.
///
/// A self-send keeps the principal: only the ZIP-317 fee leaves the wallet, and
/// the output returns as a fresh Orchard note the next run can spend.
#[event(scheduled)]
async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    match run_heartbeat(&env).await {
        Ok(Some(txid)) => {
            console_log!("heartbeat: broadcast self-send txid={txid}");
            record_heartbeat(&env, "ok", Some(&txid), None).await;
        }
        Ok(None) => console_log!("heartbeat: signer not configured; skipped"),
        Err(e) => {
            let msg = e.to_string();
            console_error!("heartbeat failed: {msg}");
            record_heartbeat(&env, "error", None, Some(&msg)).await;
        }
    }
}

/// Upsert the heartbeat result row (id = 1) read by the services status card.
/// Best-effort: a logging/D1 failure here must not turn a successful self-send
/// into a reported failure, so errors are logged and swallowed.
async fn record_heartbeat(env: &Env, status: &str, txid: Option<&str>, err: Option<&str>) {
    if let Err(e) = record_heartbeat_inner(env, status, txid, err).await {
        console_error!("heartbeat: failed to record status: {e}");
    }
}

async fn record_heartbeat_inner(
    env: &Env,
    status: &str,
    txid: Option<&str>,
    err: Option<&str>,
) -> Result<()> {
    let db = env.d1("DB")?;
    ensure_heartbeat_table(&db).await?;
    db.prepare(
        "INSERT INTO heartbeat (id, last_status, last_txid, last_error, last_run_at) \
         VALUES (1, ?1, ?2, ?3, ?4) \
         ON CONFLICT(id) DO UPDATE SET last_status = ?1, last_txid = ?2, \
         last_error = ?3, last_run_at = ?4",
    )
    .bind(&[
        js(status),
        txid.map(js).unwrap_or(JsValue::NULL),
        err.map(js).unwrap_or(JsValue::NULL),
        jsi(now_secs()),
    ])?
    .run()
    .await?;
    Ok(())
}

/// Create the heartbeat table if it does not exist. The schema is also tracked
/// as a migration (`0003_heartbeat.sql`); this lazy create lets the feature work
/// on a deploy where migrations have not been applied yet.
async fn ensure_heartbeat_table(db: &D1Database) -> Result<()> {
    db.prepare(
        "CREATE TABLE IF NOT EXISTS heartbeat ( \
         id INTEGER PRIMARY KEY CHECK (id = 1), \
         last_status TEXT NOT NULL, last_txid TEXT, last_error TEXT, \
         last_run_at INTEGER NOT NULL)",
    )
    .run()
    .await?;
    Ok(())
}

/// Perform one heartbeat self-send. Returns `Ok(Some(txid))` on success, or
/// `Ok(None)` when the signer tunnel is not configured yet (a no-op, not an
/// error, so an unconfigured deployment does not log failures every 5 minutes).
async fn run_heartbeat(env: &Env) -> Result<Option<String>> {
    let signer_base = env
        .var("SIGNER_URL")
        .map(|v| v.to_string())
        .unwrap_or_default();
    if signer_base.is_empty() || signer_base.contains("signer.invalid") {
        return Ok(None);
    }
    let secret = env.secret("SIGNER_SHARED_SECRET")?.to_string();

    // 1. Learn the faucet's own unified address (the self-send destination).
    let balance = signer_balance(&signer_base, &secret).await?;

    // 2. Self-send a tiny Orchard amount with a timestamped memo so the on-chain
    //    trail is easy to follow when auditing liveness after the fact.
    let amount_zat: u64 = env
        .var("HEARTBEAT_AMOUNT_ZAT")
        .ok()
        .and_then(|v| v.to_string().parse::<u64>().ok())
        .unwrap_or(1000);
    let memo = Some(format!("faucet-heartbeat {}", now_secs()));
    let txid = call_signer(
        &signer_base,
        &secret,
        &balance.unified_address,
        amount_zat,
        Pool::Orchard,
        memo,
    )
    .await?;
    Ok(Some(txid))
}

// ---------------------------------------------------------------------------
// Request/response payloads
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SendOtpBody {
    email: String,
}

#[derive(Deserialize)]
struct VerifyOtpBody {
    email: String,
    code: String,
}

#[derive(Deserialize)]
struct DripBody {
    address: String,
    #[serde(default)]
    memo: Option<String>,
}

// D1 row helpers.
#[derive(Deserialize)]
struct CountRow {
    n: i64,
}

#[derive(Deserialize)]
struct OtpRow {
    id: i64,
    code_hash: String,
    attempts: i64,
}

#[derive(Deserialize)]
struct SessionRow {
    email: String,
}

#[derive(Deserialize)]
struct LastDripRow {
    created_at: i64,
}

#[derive(Deserialize)]
struct StatsRow {
    n: i64,
    total: i64,
}

#[derive(Deserialize)]
struct DripHistRow {
    dest_address: String,
    pool: String,
    amount_zat: i64,
    txid: String,
    created_at: i64,
}

#[derive(Deserialize)]
struct HeartbeatRow {
    last_status: String,
    last_txid: Option<String>,
    last_error: Option<String>,
    last_run_at: i64,
}

#[derive(Deserialize)]
struct BalanceRow {
    unified_address: String,
    transparent_total_zat: i64,
    orchard_spendable_zat: i64,
    orchard_total_zat: i64,
    chain_tip: i64,
    fully_scanned: i64,
    updated_at: i64,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn handle_send_otp(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(resp) = require_basic_auth(&req, &ctx)? {
        return Ok(resp);
    }
    let body: SendOtpBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return error_json(400, "Invalid request body."),
    };
    let email = body.email.trim().to_lowercase();
    if !is_plausible_email(&email) {
        return error_json(400, "Enter a valid email address.");
    }
    // Restrict OTP recipients to an allowlist of email domains.
    let allowed = var_str(&ctx, "ALLOWED_EMAIL_DOMAINS", "zodl.com");
    if !email_domain_allowed(&email, &allowed) {
        return error_json(403, "This email domain is not allowed.");
    }

    let db = ctx.env.d1("DB")?;
    let now = now_secs();
    let ttl: i64 = var_i64(&ctx, "OTP_TTL_SECONDS", 300);

    // Throttle rapid resends: refuse if an unexpired code was issued recently.
    let recent: Option<CountRow> = db
        .prepare("SELECT COUNT(*) AS n FROM otp_codes WHERE email = ?1 AND consumed = 0 AND created_at > ?2")
        .bind(&[js(&email), jsi(now - 30)])?
        .first(None)
        .await?;
    if recent.map(|r| r.n).unwrap_or(0) > 0 {
        return error_json(429, "A code was just sent. Please wait a moment.");
    }

    let code = gen_otp()?;
    let salt = secret(&ctx, "OTP_HASH_SALT")?;
    let code_hash = sha256_hex(&salt, &code);

    db.prepare("INSERT OR IGNORE INTO users (email, created_at) VALUES (?1, ?2)")
        .bind(&[js(&email), jsi(now)])?
        .run()
        .await?;
    db.prepare("INSERT INTO otp_codes (email, code_hash, expires_at, attempts, consumed, created_at) VALUES (?1, ?2, ?3, 0, 0, ?4)")
        .bind(&[js(&email), js(&code_hash), jsi(now + ttl), jsi(now)])?
        .run()
        .await?;

    let token = secret(&ctx, "RESEND_API_TOKEN")?;
    let from = var_str(&ctx, "RESEND_FROM", "onboarding@resend.dev");
    send_otp_email(&token, &from, &email, &code).await?;

    Response::from_json(&serde_json::json!({ "message": "Code sent." }))
}

async fn handle_verify_otp(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(resp) = require_basic_auth(&req, &ctx)? {
        return Ok(resp);
    }
    let body: VerifyOtpBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return error_json(400, "Invalid request body."),
    };
    let email = body.email.trim().to_lowercase();
    let code = body.code.trim().to_string();

    let db = ctx.env.d1("DB")?;
    let now = now_secs();
    let max_attempts: i64 = var_i64(&ctx, "OTP_MAX_ATTEMPTS", 5);

    let row: Option<OtpRow> = db
        .prepare("SELECT id, code_hash, attempts FROM otp_codes WHERE email = ?1 AND consumed = 0 AND expires_at > ?2 ORDER BY created_at DESC LIMIT 1")
        .bind(&[js(&email), jsi(now)])?
        .first(None)
        .await?;
    let Some(row) = row else {
        return error_json(401, "No valid code. Request a new one.");
    };
    if row.attempts >= max_attempts {
        return error_json(429, "Too many attempts. Request a new code.");
    }

    let salt = secret(&ctx, "OTP_HASH_SALT")?;
    let provided_hash = sha256_hex(&salt, &code);
    if !ct_eq(provided_hash.as_bytes(), row.code_hash.as_bytes()) {
        db.prepare("UPDATE otp_codes SET attempts = attempts + 1 WHERE id = ?1")
            .bind(&[jsi(row.id)])?
            .run()
            .await?;
        return error_json(401, "Incorrect code.");
    }

    // Correct: consume the code and open a session.
    db.prepare("UPDATE otp_codes SET consumed = 1 WHERE id = ?1")
        .bind(&[jsi(row.id)])?
        .run()
        .await?;
    let session_ttl: i64 = var_i64(&ctx, "SESSION_TTL_SECONDS", 604_800);
    let token = gen_session_token()?;
    let token_hash = sha256_hex("session", &token);
    db.prepare(
        "INSERT INTO sessions (token_hash, email, expires_at, created_at) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(&[
        js(&token_hash),
        js(&email),
        jsi(now + session_ttl),
        jsi(now),
    ])?
    .run()
    .await?;

    let cookie = format!(
        "session={token}; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age={session_ttl}"
    );
    let headers = Headers::new();
    headers.set("Set-Cookie", &cookie)?;
    Ok(Response::from_json(&serde_json::json!({ "message": "Signed in." }))?.with_headers(headers))
}

/// Log out: delete the session server-side and clear the cookie. Idempotent.
async fn handle_logout(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(resp) = require_basic_auth(&req, &ctx)? {
        return Ok(resp);
    }
    if let Some(token) = cookie_value(&req, "session")? {
        let token_hash = sha256_hex("session", &token);
        let db = ctx.env.d1("DB")?;
        db.prepare("DELETE FROM sessions WHERE token_hash = ?1")
            .bind(&[js(&token_hash)])?
            .run()
            .await?;
    }
    let headers = Headers::new();
    headers.set(
        "Set-Cookie",
        "session=; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=0",
    )?;
    Ok(
        Response::from_json(&serde_json::json!({ "message": "Signed out." }))?
            .with_headers(headers),
    )
}

async fn handle_status(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(resp) = require_basic_auth(&req, &ctx)? {
        return Ok(resp);
    }
    let Some(email) = session_email(&req, &ctx).await? else {
        return error_json(401, "Not signed in.");
    };
    let db = ctx.env.d1("DB")?;
    let cooldown: i64 = var_i64(&ctx, "COOLDOWN_SECONDS", 86_400);
    let now = now_secs();
    let last: Option<LastDripRow> = db
        .prepare("SELECT created_at FROM drips WHERE email = ?1 ORDER BY created_at DESC LIMIT 1")
        .bind(&[js(&email)])?
        .first(None)
        .await?;
    let next_eligible = last.map(|r| r.created_at + cooldown).unwrap_or(0);
    Response::from_json(&serde_json::json!({
        "email": email,
        "eligible": now >= next_eligible,
        "next_eligible_at": next_eligible,
    }))
}

async fn handle_drip(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(resp) = require_basic_auth(&req, &ctx)? {
        return Ok(resp);
    }
    let Some(email) = session_email(&req, &ctx).await? else {
        return error_json(401, "Not signed in.");
    };
    let ip = req.headers().get("CF-Connecting-IP")?.unwrap_or_default();
    let body: DripBody = match req.json().await {
        Ok(b) => b,
        Err(_) => return error_json(400, "Invalid request body."),
    };

    // Re-validate the address server-side (the client validates too).
    let valid = match validate_destination(&body.address, Network::Testnet) {
        Ok(v) => v,
        Err(rejection) => return error_json(400, rejection.message()),
    };
    let address = body.address.trim().to_string();

    // Memos attach only to a shielded (Orchard) output; reject otherwise.
    let memo = match body
        .memo
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        None => None,
        Some(_) if valid.pool == faucet_core::Pool::Transparent => {
            return error_json(
                400,
                "Memos are only supported for shielded (Orchard / unified) addresses.",
            );
        }
        Some(m) if m.len() > faucet_core::MEMO_MAX_BYTES => {
            return error_json(400, "Memo is too long (max 512 bytes).");
        }
        Some(m) => Some(m.to_string()),
    };

    let db = ctx.env.d1("DB")?;
    let now = now_secs();
    let cooldown: i64 = var_i64(&ctx, "COOLDOWN_SECONDS", 86_400);
    let recent: Option<CountRow> = db
        .prepare("SELECT COUNT(*) AS n FROM drips WHERE created_at > ?1 AND (email = ?2 OR dest_address = ?3 OR ip = ?4)")
        .bind(&[jsi(now - cooldown), js(&email), js(&address), js(&ip)])?
        .first(None)
        .await?;
    if recent.map(|r| r.n).unwrap_or(0) > 0 {
        return error_json(429, "You already received funds recently. Try again later.");
    }

    let amount_zat: u64 = var_i64(&ctx, "DRIP_AMOUNT_ZAT", 100_000_000) as u64;

    // The transaction signer runs on a separate host, reached over a Cloudflare
    // Tunnel. If that tunnel is not wired up yet (placeholder SIGNER_URL), say
    // so explicitly instead of returning an opaque 500: the rest of the faucet
    // (auth, validation, the live balance above) works without it.
    let signer_base = var_str(&ctx, "SIGNER_URL", "");
    if signer_base.is_empty() || signer_base.contains("signer.invalid") {
        return error_json(
            503,
            "Sending is not available on this deployment yet: the transaction \
             signer (which holds the seed and builds the transaction) is not \
             connected. It runs on a separate host reached over a Cloudflare \
             Tunnel, and SIGNER_URL still points at a placeholder. The balance \
             above is live; once the tunnel is configured, drips will work.",
        );
    }
    let signer_secret = secret(&ctx, "SIGNER_SHARED_SECRET")?;
    let txid = match call_signer(
        &signer_base,
        &signer_secret,
        &address,
        amount_zat,
        valid.pool,
        memo,
    )
    .await
    {
        Ok(t) => t,
        Err(_) => {
            return error_json(
                502,
                "Could not reach the transaction signer (it may be offline or \
                 still syncing). Please try again in a few minutes.",
            );
        }
    };

    db.prepare("INSERT INTO drips (email, dest_address, pool, amount_zat, txid, ip, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")
        .bind(&[
            js(&email),
            js(&address),
            js(pool_str(valid.pool)),
            jsi(amount_zat as i64),
            js(&txid),
            js(&ip),
            jsi(now),
        ])?
        .run()
        .await?;

    Response::from_json(&DripResponse {
        txid,
        pool: valid.pool,
        amount_zat,
    })
}

// ---------------------------------------------------------------------------
// Signer call
// ---------------------------------------------------------------------------

/// Public faucet stats: total drips, total dispensed, and the most recent
/// drips (destination masked). Read from the `drips` table; behind the Basic
/// Auth gate, no session needed.
async fn handle_stats(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(resp) = require_basic_auth(&req, &ctx)? {
        return Ok(resp);
    }
    let db = ctx.env.d1("DB")?;
    let agg: Option<StatsRow> = db
        .prepare("SELECT COUNT(*) AS n, COALESCE(SUM(amount_zat), 0) AS total FROM drips")
        .first(None)
        .await?;
    let recent = db
        .prepare(
            "SELECT dest_address, pool, amount_zat, txid, created_at \
             FROM drips ORDER BY created_at DESC LIMIT 10",
        )
        .all()
        .await?;
    let rows = recent.results::<DripHistRow>()?;
    let (count, total) = agg.map(|a| (a.n, a.total)).unwrap_or((0, 0));
    let recent_json: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "address": mask_addr(&r.dest_address),
                "pool": r.pool,
                "amount_zat": r.amount_zat,
                "txid": r.txid,
                "created_at": r.created_at,
            })
        })
        .collect();
    Response::from_json(&serde_json::json!({
        "count": count,
        "total_zat": total,
        "recent": recent_json,
    }))
}

/// Mask a destination address for public display (keep the ends, hide the
/// middle). Zcash addresses are ASCII, so byte slicing is char-safe.
fn mask_addr(a: &str) -> String {
    if a.len() <= 18 {
        a.to_string()
    } else {
        format!("{}...{}", &a[..12], &a[a.len() - 6..])
    }
}

/// Public faucet reserves (behind the Basic Auth gate, no session needed).
/// Served from the cached D1 snapshot pushed by the signer host, so it works
/// without an inbound tunnel to the signer.
async fn handle_faucet_balance(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(resp) = require_basic_auth(&req, &ctx)? {
        return Ok(resp);
    }
    let db = ctx.env.d1("DB")?;
    let row: Option<BalanceRow> = db
        .prepare(
            "SELECT unified_address, transparent_total_zat, orchard_spendable_zat, \
             orchard_total_zat, chain_tip, fully_scanned, updated_at \
             FROM faucet_balance WHERE id = 1",
        )
        .first(None)
        .await?;
    match row {
        Some(r) => Response::from_json(&serde_json::json!({
            "unified_address": r.unified_address,
            "chain_tip": r.chain_tip,
            "fully_scanned": r.fully_scanned,
            "transparent_total_zat": r.transparent_total_zat,
            "orchard_spendable_zat": r.orchard_spendable_zat,
            "orchard_total_zat": r.orchard_total_zat,
            "updated_at": r.updated_at,
        })),
        None => error_json(503, "Faucet balance is not available yet."),
    }
}

/// Status of the background services behind the faucet, for the frontend status
/// card. Behind the Basic Auth gate (no session needed). Each entry has a
/// coarse `status` (`ok` | `degraded` | `down` | `not_configured` | `unknown`)
/// and a human `detail`. The signer/node states are probed live over the
/// tunnel; the heartbeat state is read from D1.
async fn handle_services(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(resp) = require_basic_auth(&req, &ctx)? {
        return Ok(resp);
    }
    let now = now_secs();
    let mut services: Vec<serde_json::Value> = Vec::new();

    // 1. Worker API: if this handler runs, the Worker is up.
    services.push(service("worker", "Worker API", "ok", "Responding"));

    // 2. Signer + 3. node (zebra + zaino), probed together via signer /info.
    let signer_base = var_str(&ctx, "SIGNER_URL", "");
    if signer_base.is_empty() || signer_base.contains("signer.invalid") {
        services.push(service(
            "signer",
            "Signer",
            "not_configured",
            "SIGNER_URL not set (tunnel not configured)",
        ));
        services.push(service(
            "node",
            "Zcash node (zebra + zaino)",
            "unknown",
            "Reached through the signer; unavailable until the signer is configured",
        ));
    } else {
        let secret = secret(&ctx, "SIGNER_SHARED_SECRET")?;
        match signer_info(&signer_base, &secret).await {
            Ok(info) => {
                services.push(service(
                    "signer",
                    "Signer",
                    "ok",
                    "Reachable; building and broadcasting transactions",
                ));
                // The node is reachable (the signer read a chain tip from it).
                // Use the cached balance snapshot to report scan progress.
                services.push(node_service(&ctx, info.chain_height).await);
            }
            Err(_) => {
                services.push(service(
                    "signer",
                    "Signer",
                    "down",
                    "Unreachable over the tunnel (offline or still starting)",
                ));
                services.push(service(
                    "node",
                    "Zcash node (zebra + zaino)",
                    "unknown",
                    "Reached through the signer, which is currently unreachable",
                ));
            }
        }
    }

    // 4. Heartbeat cron (read the last recorded result from D1).
    services.push(heartbeat_service(&ctx, now).await);

    Response::from_json(&serde_json::json!({
        "checked_at": now,
        "services": services,
    }))
}

/// Build one service entry.
fn service(key: &str, name: &str, status: &str, detail: &str) -> serde_json::Value {
    serde_json::json!({ "key": key, "name": name, "status": status, "detail": detail })
}

/// Node status derived from the live chain tip and the cached scan position.
async fn node_service(ctx: &RouteContext<()>, chain_height: u32) -> serde_json::Value {
    let name = "Zcash node (zebra + zaino)";
    let db = match ctx.env.d1("DB") {
        Ok(db) => db,
        Err(_) => return service("node", name, "ok", &format!("Chain tip {chain_height}")),
    };
    let row: Option<BalanceRow> = db
        .prepare("SELECT * FROM faucet_balance WHERE id = 1")
        .first(None)
        .await
        .ok()
        .flatten();
    match row {
        // More than 100 blocks behind the tip => still catching up.
        Some(r) if i64::from(chain_height) - r.fully_scanned > 100 => service(
            "node",
            name,
            "degraded",
            &format!("Syncing: scanned {} of tip {chain_height}", r.fully_scanned),
        ),
        Some(r) => service(
            "node",
            name,
            "ok",
            &format!("Synced: scanned {} of tip {chain_height}", r.fully_scanned),
        ),
        None => service("node", name, "ok", &format!("Chain tip {chain_height}")),
    }
}

/// Heartbeat status from its last recorded run. Considered stale if the last run
/// is older than three cron intervals (15 minutes).
async fn heartbeat_service(ctx: &RouteContext<()>, now: i64) -> serde_json::Value {
    let name = "Heartbeat cron";
    let Ok(db) = ctx.env.d1("DB") else {
        return service("heartbeat", name, "unknown", "Status unavailable");
    };
    if ensure_heartbeat_table(&db).await.is_err() {
        return service("heartbeat", name, "unknown", "Status unavailable");
    }
    let row: Option<HeartbeatRow> = db
        .prepare("SELECT * FROM heartbeat WHERE id = 1")
        .first(None)
        .await
        .ok()
        .flatten();
    let Some(r) = row else {
        return service("heartbeat", name, "unknown", "No run recorded yet");
    };
    let age = now - r.last_run_at;
    let ago = human_ago(age);
    if r.last_status == "error" {
        let msg = r.last_error.as_deref().unwrap_or("unknown error");
        return service(
            "heartbeat",
            name,
            "down",
            &format!("Last run {ago} failed: {msg}"),
        );
    }
    let txid = r.last_txid.as_deref().unwrap_or("");
    let detail = format!("Last self-send {ago} (txid {})", short_txid(txid));
    // Stale if no successful run within three intervals.
    let status = if age > 900 { "degraded" } else { "ok" };
    service("heartbeat", name, status, &detail)
}

/// Compact "N units ago" rendering for a duration in seconds.
fn human_ago(secs: i64) -> String {
    if secs < 0 {
        return "just now".to_string();
    }
    if secs < 60 {
        return format!("{secs}s ago");
    }
    if secs < 3600 {
        return format!("{}m ago", secs / 60);
    }
    if secs < 86_400 {
        return format!("{}h ago", secs / 3600);
    }
    format!("{}d ago", secs / 86_400)
}

/// First 10 chars of a txid for compact display.
fn short_txid(txid: &str) -> String {
    if txid.len() <= 10 {
        txid.to_string()
    } else {
        format!("{}...", &txid[..10])
    }
}

/// Internal: the signer host pushes a balance snapshot here (authenticated with
/// the signer shared secret, not a session). Call the Worker origin directly,
/// not the Pages proxy, so the Basic Auth gate does not apply.
async fn handle_ingest_balance(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let expected = format!("Bearer {}", secret(&ctx, "SIGNER_SHARED_SECRET")?);
    let provided = req.headers().get("Authorization")?.unwrap_or_default();
    if !ct_eq(provided.as_bytes(), expected.as_bytes()) {
        return error_json(401, "Unauthorized.");
    }
    let b: FaucetBalanceResponse = match req.json().await {
        Ok(b) => b,
        Err(_) => return error_json(400, "Invalid request body."),
    };
    let db = ctx.env.d1("DB")?;
    db.prepare(
        "INSERT INTO faucet_balance (id, unified_address, transparent_total_zat, \
         orchard_spendable_zat, orchard_total_zat, chain_tip, fully_scanned, updated_at) \
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7) \
         ON CONFLICT(id) DO UPDATE SET unified_address = ?1, transparent_total_zat = ?2, \
         orchard_spendable_zat = ?3, orchard_total_zat = ?4, chain_tip = ?5, \
         fully_scanned = ?6, updated_at = ?7",
    )
    .bind(&[
        js(&b.unified_address),
        jsi(b.transparent_total_zat as i64),
        jsi(b.orchard_spendable_zat as i64),
        jsi(b.orchard_total_zat as i64),
        jsi(i64::from(b.chain_tip)),
        jsi(i64::from(b.fully_scanned)),
        jsi(now_secs()),
    ])?
    .run()
    .await?;
    Response::from_json(&serde_json::json!({ "message": "ok" }))
}

async fn call_signer(
    signer_base: &str,
    secret: &str,
    address: &str,
    amount_zat: u64,
    pool: faucet_core::Pool,
    memo: Option<String>,
) -> Result<String> {
    let url = format!("{signer_base}/send");
    let payload = SignerSendRequest {
        address: address.to_string(),
        amount_zat,
        pool,
        memo,
    };
    let body = serde_json::to_string(&payload).map_err(|e| Error::RustError(e.to_string()))?;

    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {secret}"))?;
    headers.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&body)));
    let request = Request::new_with_init(&url, &init)?;

    let mut resp = Fetch::Request(request).send().await?;
    if resp.status_code() != 200 {
        return Err(Error::RustError(format!(
            "signer returned status {}",
            resp.status_code()
        )));
    }
    let parsed: SignerSendResponse = resp.json().await?;
    Ok(parsed.txid)
}

/// Diagnostics from the signer `/info`: confirms the signer is reachable and
/// reports the chain tip it sees from zaino (proving the node is live too).
#[derive(Deserialize)]
struct SignerInfo {
    chain_height: u32,
}

/// Probe the signer `/info` endpoint. A success proves both the signer and the
/// node (zebra + zaino) behind it are reachable. Used by the services card.
async fn signer_info(signer_base: &str, secret: &str) -> Result<SignerInfo> {
    let url = format!("{signer_base}/info");
    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {secret}"))?;
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    let request = Request::new_with_init(&url, &init)?;

    let mut resp = Fetch::Request(request).send().await?;
    if resp.status_code() != 200 {
        return Err(Error::RustError(format!(
            "signer /info returned status {}",
            resp.status_code()
        )));
    }
    resp.json().await
}

/// Fetch the faucet's own balance (and unified address) from the signer,
/// forcing a sync first. Used by the heartbeat to learn the faucet's own
/// destination address before self-sending.
async fn signer_balance(signer_base: &str, secret: &str) -> Result<FaucetBalanceResponse> {
    let url = format!("{signer_base}/balance?sync=1");
    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {secret}"))?;
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    let request = Request::new_with_init(&url, &init)?;

    let mut resp = Fetch::Request(request).send().await?;
    if resp.status_code() != 200 {
        return Err(Error::RustError(format!(
            "signer /balance returned status {}",
            resp.status_code()
        )));
    }
    resp.json().await
}

// ---------------------------------------------------------------------------
// Resend email
// ---------------------------------------------------------------------------

async fn send_otp_email(token: &str, from: &str, to: &str, code: &str) -> Result<()> {
    let html = format!(
        "<h1>Your Zcash faucet code</h1><p>Your code is: <strong>{code}</strong></p>\
         <p>It expires in 5 minutes.</p>"
    );
    let payload = serde_json::json!({
        "from": from,
        "to": [to],
        "subject": "Your Zcash faucet code",
        "html": html,
    });
    let body = serde_json::to_string(&payload).map_err(|e| Error::RustError(e.to_string()))?;

    let headers = Headers::new();
    headers.set("Authorization", &format!("Bearer {token}"))?;
    headers.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&body)));
    let request = Request::new_with_init("https://api.resend.com/emails", &init)?;

    let resp = Fetch::Request(request).send().await?;
    if resp.status_code() >= 300 {
        // Do not echo the provider response body (it may contain the address).
        return Err(Error::RustError(format!(
            "email provider returned status {}",
            resp.status_code()
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Auth helpers
// ---------------------------------------------------------------------------

/// Returns `Some(401)` if the shared HTTP Basic Auth gate fails, else `None`.
fn require_basic_auth(req: &Request, ctx: &RouteContext<()>) -> Result<Option<Response>> {
    let expected_b64 = match ctx.env.secret("BASIC_AUTH_B64") {
        Ok(s) => s.to_string(),
        // If the gate is unconfigured, fail closed.
        Err(_) => return Ok(Some(unauthorized_basic()?)),
    };
    let provided = req
        .headers()
        .get("Authorization")?
        .and_then(|h| h.strip_prefix("Basic ").map(str::to_string));
    match provided {
        Some(b64) if ct_eq(b64.as_bytes(), expected_b64.as_bytes()) => Ok(None),
        _ => Ok(Some(unauthorized_basic()?)),
    }
}

fn unauthorized_basic() -> Result<Response> {
    let headers = Headers::new();
    headers.set("WWW-Authenticate", "Basic realm=\"faucet\"")?;
    Ok(Response::error("Unauthorized", 401)?.with_headers(headers))
}

/// Resolve the signed-in email from the `session` cookie, if the session is
/// valid and unexpired.
async fn session_email(req: &Request, ctx: &RouteContext<()>) -> Result<Option<String>> {
    let Some(token) = cookie_value(req, "session")? else {
        return Ok(None);
    };
    let token_hash = sha256_hex("session", &token);
    let db = ctx.env.d1("DB")?;
    let row: Option<SessionRow> = db
        .prepare("SELECT email FROM sessions WHERE token_hash = ?1 AND expires_at > ?2")
        .bind(&[js(&token_hash), jsi(now_secs())])?
        .first(None)
        .await?;
    Ok(row.map(|r| r.email))
}

fn cookie_value(req: &Request, name: &str) -> Result<Option<String>> {
    let Some(cookies) = req.headers().get("Cookie")? else {
        return Ok(None);
    };
    for part in cookies.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix(&format!("{name}=")) {
            return Ok(Some(rest.to_string()));
        }
    }
    Ok(None)
}

// ---------------------------------------------------------------------------
// Small utilities
// ---------------------------------------------------------------------------

fn now_secs() -> i64 {
    (Date::now().as_millis() / 1000) as i64
}

fn rand_bytes<const N: usize>() -> Result<[u8; N]> {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).map_err(|e| Error::RustError(e.to_string()))?;
    Ok(buf)
}

fn gen_otp() -> Result<String> {
    let b = rand_bytes::<4>()?;
    let n = u32::from_le_bytes(b) % 1_000_000;
    Ok(format!("{n:06}"))
}

fn gen_session_token() -> Result<String> {
    let b = rand_bytes::<32>()?;
    Ok(hex::encode(b))
}

fn sha256_hex(salt: &str, data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update([0u8]);
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

/// Constant-time byte comparison (avoids leaking match length via timing).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn is_plausible_email(email: &str) -> bool {
    let bytes = email.as_bytes();
    email.len() >= 3
        && email.len() <= 254
        && email.matches('@').count() == 1
        && !email.starts_with('@')
        && !email.ends_with('@')
        && email.contains('.')
        && !bytes.iter().any(u8::is_ascii_whitespace)
}

/// Whether the email's domain is in the comma-separated allowlist.
fn email_domain_allowed(email: &str, allowed_csv: &str) -> bool {
    let domain = email.rsplit('@').next().unwrap_or("");
    allowed_csv
        .split(',')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .any(|d| d.eq_ignore_ascii_case(domain))
}

fn pool_str(pool: faucet_core::Pool) -> &'static str {
    match pool {
        faucet_core::Pool::Transparent => "transparent",
        faucet_core::Pool::Orchard => "orchard",
    }
}

fn error_json(status: u16, msg: &str) -> Result<Response> {
    Ok(Response::from_json(&serde_json::json!({ "error": msg }))?.with_status(status))
}

// Bind helpers for D1 prepared statements.
fn js(s: &str) -> JsValue {
    JsValue::from_str(s)
}

fn jsi(n: i64) -> JsValue {
    JsValue::from_f64(n as f64)
}

fn secret(ctx: &RouteContext<()>, name: &str) -> Result<String> {
    Ok(ctx.env.secret(name)?.to_string())
}

fn var_str(ctx: &RouteContext<()>, name: &str, default: &str) -> String {
    ctx.env
        .var(name)
        .map(|v| v.to_string())
        .unwrap_or_else(|_| default.to_string())
}

fn var_i64(ctx: &RouteContext<()>, name: &str, default: i64) -> i64 {
    ctx.env
        .var(name)
        .ok()
        .and_then(|v| v.to_string().parse::<i64>().ok())
        .unwrap_or(default)
}
