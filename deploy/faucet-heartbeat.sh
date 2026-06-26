#!/bin/sh
#
# Faucet heartbeat: create one on-chain transaction so the chain keeps moving
# and the whole pipeline (sync -> build -> prove -> broadcast) is exercised on a
# schedule. Run periodically by the `heartbeat` docker-compose sidecar (every
# HEARTBEAT_INTERVAL seconds) or by a host crontab.
#
# What it does: the faucet sends a tiny Orchard amount to its OWN unified
# address (a self-send). This produces a real testnet transaction every run,
# but keeps the principal: only the ZIP-317 fee leaves the wallet, and the
# output (minus fee) returns as a fresh Orchard note the next run can spend.
# If the faucet ever stops building, proving, or broadcasting, these runs fail
# loudly in the log, which is the point: a long-running liveness probe.
#
# This is intentionally a single drip per invocation (like
# faucet-maintenance.sh), so it drops cleanly into cron:
#
#   */5 * * * * SIGNER_SHARED_SECRET=... \
#     /opt/faucet/deploy/faucet-heartbeat.sh >> /var/log/faucet-heartbeat.log 2>&1
#
# POSIX sh (no bashisms) so it also runs under busybox in the curl sidecar.
#
# Environment:
#   SIGNER_LOCAL_URL      signer base URL (default http://127.0.0.1:8080)
#   SIGNER_SHARED_SECRET  bearer secret shared by the signer and Worker (required)
#   HEARTBEAT_AMOUNT_ZAT  self-send amount in zatoshis (default 1000 = 0.00001 TAZ)
#   HEARTBEAT_MEMO_PREFIX memo prefix for the Orchard output (default "faucet-heartbeat")
#
# Note: this does not serialize against an in-flight drip or the maintenance
# shield. Keep the interval coarse (minutes) on a low-traffic testnet faucet.

set -eu

signer="${SIGNER_LOCAL_URL:-http://127.0.0.1:8080}"
secret="${SIGNER_SHARED_SECRET:?set SIGNER_SHARED_SECRET}"
amount="${HEARTBEAT_AMOUNT_ZAT:-1000}"
memo_prefix="${HEARTBEAT_MEMO_PREFIX:-faucet-heartbeat}"
auth="Authorization: Bearer ${secret}"

# A UTC timestamp keeps each heartbeat memo distinct and makes the on-chain
# trail easy to follow when auditing liveness after the fact.
now="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
memo="${memo_prefix} ${now}"

# 1. Discover the faucet's own unified address (sync first so the address and
#    spendable notes are current). The signer returns compact JSON, so a simple
#    field extraction avoids a jq dependency in the sidecar image.
balance="$(curl -fsS "${signer}/balance?sync=1" -H "${auth}")"
address="$(printf '%s' "${balance}" \
    | sed -n 's/.*"unified_address":"\([^"]*\)".*/\1/p')"

if [ -z "${address}" ]; then
    echo "heartbeat: could not read faucet unified_address from /balance" >&2
    echo "heartbeat: balance payload was: ${balance}" >&2
    exit 1
fi

# 2. Self-send a tiny Orchard amount with the timestamped memo. The signer
#    builds, proves, and broadcasts; on success it returns {"txid":"..."}.
resp="$(curl -fsS -X POST "${signer}/send" \
    -H "${auth}" -H 'Content-Type: application/json' \
    -d "{\"address\":\"${address}\",\"amount_zat\":${amount},\"pool\":\"orchard\",\"memo\":\"${memo}\"}")"

txid="$(printf '%s' "${resp}" | sed -n 's/.*"txid":"\([^"]*\)".*/\1/p')"

if [ -z "${txid}" ]; then
    echo "heartbeat: send did not return a txid: ${resp}" >&2
    exit 1
fi

echo "heartbeat: broadcast ${amount} zat self-send to ${address} (memo: ${memo}) txid=${txid}"
