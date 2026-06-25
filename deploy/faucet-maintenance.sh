#!/usr/bin/env bash
#
# Periodic faucet maintenance, run by cron/systemd timer on the signer host.
# It does two things, in order:
#
#   1. Shields matured transparent funds (the faucet is fed by mining coinbase)
#      into the Orchard pool, so they become spendable by drips. Shielding only
#      selects coinbase with >= 100 confirmations, so immature rewards are
#      skipped; a run with nothing to shield is a no-op.
#   2. Refreshes the cached balance: it reads the per-pool balance from the
#      signer and pushes it to the Worker (POST /api/internal/balance), so the
#      frontend "Faucet reserves" panel stays current without an inbound tunnel.
#
# Environment:
#   SIGNER_LOCAL_URL      signer base URL on the host (default http://127.0.0.1:8080)
#   WORKER_URL            Worker origin, e.g. https://<name>.workers.dev (required)
#   SIGNER_SHARED_SECRET  bearer secret shared by the signer and the Worker (required)
#
# Example crontab (every 10 minutes):
#   */10 * * * * SIGNER_SHARED_SECRET=... WORKER_URL=https://faucet-zcash-api.example.workers.dev \
#     /opt/faucet/deploy/faucet-maintenance.sh >> /var/log/faucet-maintenance.log 2>&1
#
# Note: this serializes nothing against an in-flight drip; keep the interval
# coarse (minutes) on a low-traffic testnet faucet, or serialize in the signer.

set -euo pipefail

signer="${SIGNER_LOCAL_URL:-http://127.0.0.1:8080}"
worker="${WORKER_URL:?set WORKER_URL to the Worker origin}"
secret="${SIGNER_SHARED_SECRET:?set SIGNER_SHARED_SECRET}"
auth="Authorization: Bearer ${secret}"

# 1. Shield matured coinbase. A failure here (e.g. nothing mature yet) must not
#    abort the balance refresh, so it is tolerated.
if curl -fsS -X POST "${signer}/shield" -H "${auth}" -o /dev/null; then
    echo "shield: ok"
else
    echo "shield: nothing to shield or transient error (continuing)"
fi

# 2. Refresh the cached balance (sync, then push to the Worker).
balance="$(curl -fsS "${signer}/balance?sync=1" -H "${auth}")"
curl -fsS -X POST "${worker}/api/internal/balance" \
    -H "${auth}" -H 'Content-Type: application/json' \
    -d "${balance}" -o /dev/null
echo "balance: pushed to ${worker}"
