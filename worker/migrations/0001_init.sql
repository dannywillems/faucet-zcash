-- Initial D1 schema for the Zcash testnet faucet.
-- Times are unix seconds (INTEGER).

CREATE TABLE IF NOT EXISTS users (
    email      TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS otp_codes (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    email      TEXT NOT NULL,
    code_hash  TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    attempts   INTEGER NOT NULL DEFAULT 0,
    consumed   INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_otp_email ON otp_codes (email, created_at);

CREATE TABLE IF NOT EXISTS sessions (
    token_hash TEXT PRIMARY KEY,
    email      TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS drips (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    email        TEXT NOT NULL,
    dest_address TEXT NOT NULL,
    pool         TEXT NOT NULL,
    amount_zat   INTEGER NOT NULL,
    txid         TEXT NOT NULL,
    ip           TEXT,
    created_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_drips_email_time ON drips (email, created_at);
CREATE INDEX IF NOT EXISTS idx_drips_addr_time ON drips (dest_address, created_at);
CREATE INDEX IF NOT EXISTS idx_drips_ip_time ON drips (ip, created_at);
