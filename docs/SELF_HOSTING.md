# Oudos Self-Hosting Guide (3-Minute Quickstart)

Oudos is 100% **GNU AGPL-3.0** self-hostable with **zero paywalled features**. The complete web console (`apps/web-pwa`), device registry, monotonic policy engine, 15-minute one-time support links, and tamper-evident hash-chained CSV audit log boot from a single `docker compose` command.

---

## 1. Prerequisites

- **Docker Engine + Docker Compose v2** (tested target: Ubuntu 24.04 / WSL2)
- Optional for production domains: DNS A/AAAA record pointing `support.example.org` to your host (for automatic HTTPS via `--profile tls`)
- Optional for strict symmetric-NAT networks: either self-hosted `coturn` (`--profile coturn`) or Cloudflare Realtime TURN (1,000 GB/month free tier)

---

## 2. 1-Command Quickstart (Under 3 Minutes)

From the repository root:

```bash
cp .env.example deploy/.env
docker compose -f deploy/docker-compose.yml up -d --build
```

Verify the control plane and web console are healthy:

```bash
curl -fsS http://localhost:8080/health
curl -fsS http://localhost:8080/api/v1/server-info
```

Open `http://localhost:8080` in your browser to access the **Devices**, **Sessions**, **Policies**, and **Audit Log** console.

> [!NOTE]
> `OUD_SERVER_SEED_HEX` can be left blank on first boot. `oud-server` automatically generates an Ed25519 identity keypair at `/var/lib/oudos/server.key` inside the persistent `oudos_data` volume so your server's public key fingerprint stays unchanged across restarts.

---

## 3. Transport & TURN Options ($0 Budget)

By default (`OUD_TURN_MODE=direct_p2p`), peers establish direct end-to-end encrypted WebRTC connections using `stun:stun.cloudflare.com:3478` at zero cost.

### Option A — Self-Hosted `coturn` Fallback
1. Set in `deploy/.env`:
   ```env
   OUD_TURN_MODE=coturn
   OUD_COTURN_HOST=turn.example.org
   OUD_COTURN_SECRET=replace-with-random-secret
   ```
2. Start with the `coturn` profile:
   ```bash
   docker compose -f deploy/docker-compose.yml --profile coturn up -d --build
   ```

### Option B — Cloudflare Realtime TURN Free Tier (1,000 GB/month)
Set in `deploy/.env`:
```env
OUD_TURN_MODE=cloudflare
OUD_CF_TURN_TOKEN_ID=your-turn-token-id
OUD_CF_TURN_API_TOKEN=your-turn-api-token
```
*(Secrets stay on the server; `/api/v1/server-info` never exposes `OUD_COTURN_SECRET` or `OUD_CF_TURN_API_TOKEN`.)*

---

## 4. Production HTTPS (`--profile tls`)

PWAs and WebAuthn/passkeys require HTTPS on non-localhost domains. Set `OUD_SERVER_DOMAIN=support.example.org` in `deploy/.env` and launch the included Caddy reverse proxy:

```bash
docker compose -f deploy/docker-compose.yml --profile tls up -d --build
```

---

## 5. Pre-Paired Host Enrollment Flow

1. Query `GET /api/v1/server-info` (or copy the Ed25519 key shown in container logs on startup).
2. Enroll your server's raw domain (`support.example.org`) and Ed25519 public key on each Windows host agent (`oud-host-desktop`).
3. Every host consent card displays your raw enrolled domain—unknown servers are blocked fail-closed.

---

## 6. Verifying Audit CSV Export

From the **Audit Log** tab in the web console (or via `GET /api/v1/audit/export.csv` with a valid bearer token), download the hash-chained CSV export (`sequence, event_id, timestamp_utc, ..., prev_hash, entry_hash, host_signature`). Each entry contains metadata only—never screen pixels, keystrokes, clipboard text, or file contents.
