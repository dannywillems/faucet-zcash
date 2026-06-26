# Infrastructure

How the faucet is deployed, and in particular how a Cloudflare Worker reaches a
signer that runs on a private machine with **no open inbound ports**, using a
Cloudflare Tunnel. The last section covers the security properties of that
design.

If you only remember one thing: the signer does not listen for connections from
the internet. The signer's host dials **out** to Cloudflare and holds the
connection open; Cloudflare sends Worker requests back down that same
connection. The host needs no public IP, no port forwarding, and no firewall
hole.

## Components

| Component         | Where it runs                    | Role                                                                 |
| ----------------- | -------------------------------- | ------------------------------------------------------------------- |
| Frontend          | Cloudflare Pages                 | SvelteKit static SPA; in-browser address validation (wasm).         |
| Worker API + D1   | Cloudflare Workers + D1          | Public API. Basic Auth gate, email OTP, sessions, drip endpoint. Holds no seed. |
| Signer            | A private host (your machine)    | Native Rust (axum). Holds the seed; builds, proves, broadcasts.     |
| `cloudflared`     | Same private host as the signer  | Cloudflare Tunnel daemon. Connects the signer to Cloudflare.        |
| Upstream lightwalletd | Public `testnet.zec.rocks` (default) or self-hosted zaino + zebrad | The signer syncs and broadcasts through it. |

Heavy crypto (Orchard halo2 proving) needs native, multi-threaded CPU it cannot
get inside a Worker, which is why the signer is a separate native process rather
than part of the Worker.

## How a request reaches the signer (Cloudflare Tunnel)

```
  Browser
    |  HTTPS
    v
  Cloudflare edge ----> Pages (static) and Worker (/api/*)
                              |
                              |  Worker calls SIGNER_URL (HTTPS + Bearer secret)
                              v
                        Cloudflare edge
                              |
                              |  routes the request DOWN the tunnel
                              |  (the connection cloudflared opened outbound)
                              v
        +-------------------------------------------+
        |  Your private host                        |
        |                                           |
        |   cloudflared  --(outbound, kept open)--> | to Cloudflare
        |       |                                   |
        |       v  http://signer:8080               |
        |   signer (holds seed, builds tx)          |
        |       |                                   |
        |       v  gRPC/TLS                          |
        |   upstream lightwalletd (public)          |
        +-------------------------------------------+
```

Step by step:

1. On the host, `cloudflared` makes an **outbound** TLS connection to Cloudflare
   and keeps it open. It registers a hostname (the value behind the Worker's
   `SIGNER_URL` secret).
2. When the Worker needs the signer (a drip, the heartbeat, a balance refresh),
   it makes an HTTPS request to `SIGNER_URL` with an `Authorization: Bearer
   <SIGNER_SHARED_SECRET>` header.
3. Cloudflare's edge matches the hostname to the open tunnel and forwards the
   request down it to `http://signer:8080` on the host. No inbound connection is
   ever made to the host.
4. The signer checks the bearer secret, does the work (sync, build, prove,
   broadcast), and replies back up the same tunnel.

Because the connection is established from the host outward, the host needs no
public IP, no inbound firewall rule, and no port forwarding.

### What happens when the host is down

If the host (or just `cloudflared` / the signer container) stops, the tunnel has
no origin. Cloudflare then returns **HTTP 530** for requests to `SIGNER_URL`.
The faucet degrades gracefully:

- The drip path returns a clear "signer offline" message instead of a 500.
- The background-services status card shows **Signer: Down** and the reserves
  go stale.

This is the expected meaning of a 530 here: "the tunnel reached Cloudflare, but
nothing answered on the host." The fix is to bring the host back up
(`docker compose up -d`); the system recovers on its own.

## Background jobs and data flow

| Job              | Runs on               | Schedule | What it does                                                                 |
| ---------------- | --------------------- | -------- | --------------------------------------------------------------------------- |
| Heartbeat        | Cloudflare Worker cron | every 5 min | Asks the signer to self-send a tiny Orchard amount, keeping the chain moving and proving the build/prove/broadcast path. |
| Maintenance      | Host cron (`deploy/faucet-maintenance.sh`) | every 10 min | Shields matured coinbase into Orchard, reads the balance, and pushes it to the Worker (`POST /api/internal/balance`). |
| Reserves display | Worker (`GET /api/faucet/balance`) | on request | Serves the cached `faucet_balance` row written by the maintenance job. |

The maintenance push is **outbound from the host**, so the reserves panel works
even though there is no inbound path to the host. Note the consequence: because
both the signer and the maintenance cron live on the host, the faucet is live
only while that host is running. If the machine sleeps, drips fail and reserves
stop refreshing until it is back.

## Security model

### What this design protects

- **No inbound attack surface on the signer host.** The tunnel is outbound-only,
  so there are no open ports to scan or exploit, and nothing to expose through a
  home or office firewall.
- **Seed isolation.** The faucet seed exists only on the signer host, held in
  zeroized memory. It is never on Cloudflare, in the Worker, in the frontend, or
  in D1. A compromise of the Cloudflare side cannot reveal the seed.
- **Authenticated Worker-to-signer channel.** The Worker presents a bearer
  secret (`SIGNER_SHARED_SECRET`), which the signer compares in constant time.
  As defense in depth, the signer independently re-validates the destination
  address before building a transaction.
- **Gated public API.** The public surface is the Worker, fronted by an HTTP
  Basic Auth bot gate, email OTP, and a session cookie, with an optional
  per-identity drip cooldown. The signer is never directly routable; it is
  reachable only through the bearer-gated tunnel.
- **TLS in transit** end to end: browser to edge, Worker to signer (tunnel), and
  signer to lightwalletd.

### What to watch (risks and trade-offs)

- **The bearer secret is the keystone.** Anyone who learns both
  `SIGNER_SHARED_SECRET` and the tunnel hostname can ask the signer to spend (up
  to what the faucet wallet holds). Keep the hostname unpublished, rotate the
  secret, and consider Cloudflare Access (below) so unauthenticated requests
  never reach the signer at all.
- **Cloudflare is in the request path.** Cloudflare terminates TLS at its edge,
  so it can observe the Worker-to-signer payloads (destination address, amount,
  memo). This is acceptable for a testnet faucet; for sensitive or mainnet use,
  reconsider what data transits the edge.
- **Host compromise means seed compromise.** The host holds the seed, so keep it
  patched and minimal. Here the blast radius is testnet funds only, which have
  no real value.
- **Single point of failure.** There is one signer host; if it is down, drips
  fail (handled gracefully) until it returns. Use an always-on machine, or move
  the signer to managed compute, if you need uptime.

### Hardening options (optional)

- **Cloudflare Access** (a service token, or mTLS) on the tunnel hostname so the
  edge rejects unauthenticated traffic before it ever reaches the signer. This
  turns the bearer secret into a second factor rather than the only gate.
- **Rotate `SIGNER_SHARED_SECRET`** on a schedule and treat it like a signing
  key.
- **Keep the faucet wallet funded minimally** so the spendable balance caps the
  worst case.

## See also

- [`../deploy/README.md`](../deploy/README.md) for step-by-step deployment.
- [`../README.md`](../README.md) for the component overview.
