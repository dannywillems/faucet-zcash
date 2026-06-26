-- Last result of the chain-liveness heartbeat cron (a single row, id = 1).
-- The Worker's scheduled handler upserts this row on every tick; the services
-- status card (GET /api/faucet/services) reads it to show when the heartbeat
-- last ran and whether it succeeded. The Worker also creates this table lazily
-- (CREATE TABLE IF NOT EXISTS) so the feature works even before this migration
-- is applied; this file is the canonical schema and is used for local dev.
CREATE TABLE IF NOT EXISTS heartbeat (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    last_status TEXT NOT NULL,            -- 'ok' | 'error'
    last_txid   TEXT,                     -- broadcast txid when last_status = 'ok'
    last_error  TEXT,                     -- error message when last_status = 'error'
    last_run_at INTEGER NOT NULL          -- unix seconds of the last attempt
);
