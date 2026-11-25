# Oudos Self-Hosting Guide

Oudos is 100% AGPL-3.0 self-hostable with zero paywalled features. The complete control plane, web PWA console, monotonic policy engine, and hash-chained CSV audit log run from a single `docker compose up` command (packaged in `deploy/docker-compose.yml` in Issue `#11`).

## Prerequisites
- Docker Engine + Docker Compose v2 (tested on Ubuntu 24.04 / WSL2)
- Public TLS reverse proxy (e.g., Caddy or Nginx) for WebSockets and PWA installability
- Optional: Self-hosted `coturn` or Cloudflare Realtime TURN credentials for strict symmetric-NAT relay fallback

## Planned Stack (`deploy/docker-compose.yml`)
- `oud-server`: Rust (Axum) control plane, SQLite/PostgreSQL store, WebSocket signaling, audit API
- `web-pwa`: Static React + TypeScript PWA served over HTTPS
- `coturn` (optional profile): Standards-compliant TURN/STUN relay

## Host Enrollment Flow
1. Deploy `oud-server` on your domain (e.g., `support.example.org`).
2. Obtain the server's Ed25519 public key fingerprint from the admin console.
3. Explicitly enroll that server public key on each Windows host agent before any remote consent prompt from that server is accepted.
