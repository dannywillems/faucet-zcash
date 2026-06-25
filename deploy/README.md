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
# set SIGNER_URL in wrangler.toml to the tunnel hostname + /api is NOT needed;
# the signer endpoint is its own hostname, e.g. https://signer.example/send
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

The signer syncs and broadcasts through a local **zaino** (which serves the
lightwalletd gRPC protocol), backed by **zebrad** on testnet. Run both on the
host (zebrad fully synced to testnet, zaino pointed at zebrad's RPC), then set
`LIGHTWALLETD_URL` to zaino's gRPC address.

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

> Status: the signer opens its wallet DB, runs migrations, and derives the
> faucet account from the seed on first run (verified by the integration
> tests). The live sync + proposal + Orchard/transparent proving + broadcast
> against zebra + zaino is the remaining step (`signer/src/wallet.rs`); until
> it is enabled `/send` returns 503. Everything else (auth, OTP, cooldown, UI)
> is functional.
