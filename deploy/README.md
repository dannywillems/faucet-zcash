# Deployment

Three pieces: the SvelteKit frontend on Cloudflare Pages, the Rust Worker + D1,
and the signer on your own host (reached by the Worker over a Cloudflare
Tunnel). The frontend and Worker share one origin via a `/api/*` route so the
Basic Auth gate and the session cookie work.

## 1. D1 database

```bash
cd worker
npx wrangler d1 create faucet-zcash
# copy the database_id into worker/wrangler.toml
npx wrangler d1 migrations apply faucet-zcash --remote
```

## 2. Worker secrets

```bash
cd worker
npx wrangler secret put RESEND_API_TOKEN
npx wrangler secret put BASIC_AUTH_B64          # base64("user:pass")
npx wrangler secret put SIGNER_SHARED_SECRET
npx wrangler secret put OTP_HASH_SALT
# In wrangler.toml set SIGNER_URL to the signer's tunnel BASE URL (no path);
# the Worker appends /send and /balance, e.g. https://signer.example
```

Deploy the Worker (CI does this on push to main, or manually):

```bash
make deploy-worker
```

Add a Worker route so `<your-domain>/api/*` maps to this Worker, while Pages
serves everything else on the same domain.

## 3. Frontend (Pages)

One-time project creation:

```bash
npx wrangler pages project create faucet-zcash --production-branch main
```

Set the Pages project's `BASIC_AUTH_B64` variable to the same value as the
Worker. Deploy (CI does this on push to main):

```bash
make deploy-pages
```

GitHub Actions needs repo secrets `CLOUDFLARE_API_TOKEN` and
`CLOUDFLARE_ACCOUNT_ID`.

## 4. Node: zebra + zaino (your host)

The signer syncs and broadcasts through a lightwalletd-protocol server. By
default it uses the public `https://testnet.zec.rocks:443` (the server the
zodl mobile apps use), so no local node is required to get started. To run your
own, bring up **zaino** (which serves the lightwalletd gRPC protocol) backed by
**zebrad** on testnet, and set `LIGHTWALLETD_URL` to zaino's address (e.g.
`http://127.0.0.1:8137`). The signer enables TLS automatically for `https://`
endpoints.

The faucet is typically funded by mining rewards, which arrive as **transparent
coinbase**. Those cannot be sent directly; the signer shields them into the
Orchard pool (only coinbase with >= 100 confirmations is selected) before they
can be dripped. See the maintenance cron below to automate this.

## 5. Signer (your host)

```bash
cd deploy
cp .env.example .env   # fill SIGNER_SHARED_SECRET, SIGNER_SEED, LIGHTWALLETD_URL, tunnel token
docker compose up -d
```

The `cloudflared` service connects out to Cloudflare; configure the tunnel in
the Zero Trust dashboard to route your signer hostname to `http://signer:8080`.
The signer is never published on a host port. The containerized signer reaches
zaino on the host via `host.docker.internal`; the wallet DB persists on the
`faucet-data` volume.

## 6. Maintenance cron (auto-shield + balance)

`faucet-maintenance.sh` shields matured coinbase into Orchard and pushes the
faucet's per-pool balance to the Worker (`POST /api/internal/balance`), which
serves it to the frontend "Faucet reserves" panel. This push is **outbound**
from the host, so the balance works even without the inbound tunnel. Run it on
a timer:

```bash
# crontab on the signer host, every 10 minutes:
*/10 * * * * SIGNER_SHARED_SECRET=... \
  WORKER_URL=https://faucet-zcash-api.<acct>.workers.dev \
  /opt/faucet/deploy/faucet-maintenance.sh >> /var/log/faucet-maintenance.log 2>&1
```

`WORKER_URL` is the Worker origin (`*.workers.dev`), not the Pages domain, so
the push bypasses the Basic Auth gate (it authenticates with the signer
secret).

## 7. Heartbeat cron (chain liveness)

`faucet-heartbeat.sh` makes the faucet send a tiny Orchard amount to its **own**
unified address every few minutes. This is a liveness probe: each run produces a
real testnet transaction and exercises the full `sync -> build -> prove ->
broadcast` path, so if the pipeline ever rots (sync stalls, prover params
missing, zaino unreachable), the runs fail loudly in the log. It is a self-send,
so only the ZIP-317 fee leaves the wallet; the output returns as a fresh Orchard
note the next run can spend.

The heartbeat runs automatically as a `heartbeat` sidecar in
`docker-compose.yml`, so `docker compose up -d` (or `make stack-up`) starts it
alongside the signer. It reaches the signer over the internal Docker network
(never a host port). Tune it in `.env`:

```bash
HEARTBEAT_INTERVAL=300      # seconds between transactions (default 5 minutes)
HEARTBEAT_AMOUNT_ZAT=1000   # self-send note size in zatoshis (default 0.00001 TAZ)
```

To check it is alive:

```bash
docker compose logs -f heartbeat
# heartbeat: broadcast 1000 zat self-send to utest1... txid=...
```

Alternatively, run it from a host crontab instead of the sidecar (omit the
service or set a long interval), every 5 minutes:

```bash
*/5 * * * * SIGNER_SHARED_SECRET=... \
  /opt/faucet/deploy/faucet-heartbeat.sh >> /var/log/faucet-heartbeat.log 2>&1
```

The heartbeat needs spendable Orchard notes, so it only works after the faucet
is funded and the maintenance shield (step 6) has run at least once.

## Status

The signer wallet engine is implemented end-to-end against the lightwalletd
protocol: derive account, sync (with 429 back-off and TLS), shield transparent
coinbase into Orchard, build + prove (Orchard halo2 / transparent secp256k1) +
broadcast, and report per-pool balance (`signer/src/wallet.rs`). The drip path
(`POST /api/faucet/drip` -> Worker -> signer `/send`) needs the inbound tunnel
(`SIGNER_URL`) configured; the balance panel does not (it uses the outbound
push above).

Known limitation: a light client cannot tell from the lightwalletd protocol
whether a transparent UTXO is coinbase, so coinbase maturity is enforced via a
100-confirmation policy on transparent inputs rather than the coinbase flag.
