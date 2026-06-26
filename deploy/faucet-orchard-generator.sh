#!/bin/sh
#
# Orchard activity generator. Fires a burst of small Orchard self-send
# transactions so the Orchard pool stays continuously active. Runs on the signer
# host (behind the Cloudflare Tunnel), talking to the local signer directly, from
# a per-minute cron:
#
#   * * * * * SIGNER_SHARED_SECRET=... \
#     /opt/faucet/deploy/faucet-orchard-generator.sh \
#     >> /var/log/faucet-orchard-generator.log 2>&1
#
# Each run sends ORCHARD_TX_BURST (default 10) tiny Orchard amounts to the
# faucet's own unified address. Self-sends keep the principal; only the ZIP-317
# fee leaves the wallet per transaction.
#
# Note on throughput: each Orchard send spends a confirmed note and produces
# unconfirmed change, so the number that can land in one minute is bounded by how
# many confirmed spendable notes the wallet has. Sends beyond that fail until the
# change from earlier ones confirms; failures are tolerated and the burst
# continues. Over time the self-sends fragment the balance into more notes, which
# raises the achievable rate.
#
# Environment:
#   SIGNER_LOCAL_URL      signer base URL (default http://127.0.0.1:8080)
#   SIGNER_SHARED_SECRET  bearer secret shared with the signer (required)
#   ORCHARD_TX_BURST      transactions per run (default 10)
#   ORCHARD_TX_AMOUNT_ZAT self-send amount in zatoshis (default 1000)
#
# POSIX sh (no bashisms).

set -eu

signer="${SIGNER_LOCAL_URL:-http://127.0.0.1:8080}"
secret="${SIGNER_SHARED_SECRET:?set SIGNER_SHARED_SECRET}"
burst="${ORCHARD_TX_BURST:-10}"
amount="${ORCHARD_TX_AMOUNT_ZAT:-1000}"
auth="Authorization: Bearer ${secret}"

# Single-flight: proving a burst can outlast the one-minute cron interval, so
# skip this run if a previous one is still active. mkdir is atomic.
lock="${TMPDIR:-/tmp}/faucet-orchard-generator.lock"
if ! mkdir "$lock" 2>/dev/null; then
    echo "$(date -u +%H:%M:%S) previous run still active; skipping"
    exit 0
fi
trap 'rmdir "$lock" 2>/dev/null || true' EXIT

# The faucet's own unified address is the self-send target. Sync once up front so
# the whole burst spends from a current view of the wallet.
balance="$(curl -fsS "${signer}/balance?sync=1" -H "${auth}")"
address="$(printf '%s' "$balance" | sed -n 's/.*"unified_address":"\([^"]*\)".*/\1/p')"
if [ -z "$address" ]; then
    echo "$(date -u +%H:%M:%S) could not read faucet unified_address; aborting" >&2
    exit 1
fi

ok=0
fail=0
i=1
while [ "$i" -le "$burst" ]; do
    memo="orchard-gen $(date -u +%Y-%m-%dT%H:%M:%SZ) #${i}"
    # A per-transaction failure (e.g. no confirmed note available yet) must not
    # abort the rest of the burst.
    resp="$(curl -fsS -X POST "${signer}/send" \
        -H "${auth}" -H 'Content-Type: application/json' \
        -d "{\"address\":\"${address}\",\"amount_zat\":${amount},\"pool\":\"orchard\",\"memo\":\"${memo}\"}" \
        2>/dev/null || true)"
    txid="$(printf '%s' "$resp" | sed -n 's/.*"txid":"\([^"]*\)".*/\1/p')"
    if [ -n "$txid" ]; then
        ok=$((ok + 1))
        echo "[$i/$burst] ok txid=${txid}"
    else
        fail=$((fail + 1))
        echo "[$i/$burst] FAIL ${resp}"
    fi
    i=$((i + 1))
done
echo "$(date -u +%H:%M:%S) burst done: ${ok} ok, ${fail} failed (${amount} zat each)"
