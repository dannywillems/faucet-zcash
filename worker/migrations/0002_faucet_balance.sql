-- Cached snapshot of the faucet's on-chain reserves (a single row, id = 1).
-- The signer host pushes snapshots outbound to the Worker (POST
-- /api/internal/balance), so the frontend can show reserves without an inbound
-- tunnel to the signer. GET /api/faucet/balance serves this row.
CREATE TABLE IF NOT EXISTS faucet_balance (
    id                    INTEGER PRIMARY KEY CHECK (id = 1),
    unified_address       TEXT NOT NULL,
    transparent_total_zat INTEGER NOT NULL,
    orchard_spendable_zat INTEGER NOT NULL,
    orchard_total_zat     INTEGER NOT NULL,
    chain_tip             INTEGER NOT NULL,
    fully_scanned         INTEGER NOT NULL,
    updated_at            INTEGER NOT NULL
);
