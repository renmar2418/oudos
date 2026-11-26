# Oudos Website (`site/`)

Cloudflare Workers (Static Assets + optional Turnstile contact endpoint) showcase and download site for **Oudos** (`v0.1.0-rc.1`, GNU AGPL-3.0).

## Layout

- `wrangler.jsonc` — Worker configuration (`oudos-site`, `ASSETS` binding, `404-page`, `run_worker_first: ["/api/*"]`)
- `src/index.js` — `GET /api/config`, `POST /api/contact` (Turnstile Siteverify + optional webhook forwarding), and static fallback
- `public/` — Static HTML/CSS/JS, `_headers`, `404.html`, extracted brand SVGs, and `v0.1.0-rc.1` showcase PNGs

## Local Preview

Static-only preview (contact form automatically displays GitHub Issues fallback):

```bash
python -m http.server 8788 --directory site/public
```

Full Worker preview (from `site/`):

```bash
npx wrangler dev
```

## Deploy to Cloudflare Workers

### Option A — CLI Deploy (`wrangler`)

```bash
cd site
npx wrangler deploy
```

### Option B — Cloudflare Workers Git Integration

In the Cloudflare dashboard (**Workers & Pages -> Create -> Import a repository**):

1. Set **Root directory** to `site`.
2. Leave build command blank (pure static assets + vanilla JS Worker) and deploy command as `npx wrangler deploy`.

## Custom Domain & DNS Cutover (`oudos-site.renmar.dev`)

### 1. Cloudflare Nameserver Cutover (`renmar.dev`)

1. Add `renmar.dev` to your Cloudflare account.
2. At your domain registrar, replace existing nameservers for `renmar.dev` with Cloudflare's assigned pair:
   - `arushi.ns.cloudflare.com`
   - `will.ns.cloudflare.com`

### 2. Attach Worker Custom Domain

1. Open **Workers & Pages -> `oudos` -> Settings -> Domains & Routes -> Add -> Custom Domain** (note: `wrangler.jsonc` specifies `"name": "oudos-site"` when deploying directly via `npx wrangler deploy`).
2. Add `oudos-site.renmar.dev`. Cloudflare provisions DNS and edge TLS automatically once the zone is active.
3. `.workers.dev` fallback endpoint: `https://oudos.gombio2418.workers.dev`

### 3. Verify DNS Propagation

Run on Windows after registrar cutover:

```powershell
nslookup -type=NS renmar.dev 1.1.1.1
ipconfig /flushdns
nslookup oudos-site.renmar.dev 1.1.1.1
```

Confirm `renmar.dev` reports `arushi.ns.cloudflare.com` and `will.ns.cloudflare.com`, and `oudos-site.renmar.dev` resolves to Cloudflare edge addresses.

### Optional Turnstile + Contact Endpoint

The site works with zero secrets configured (downloads are never gated, and the contact section falls back to GitHub Issues). To enable the live `POST /api/contact` form:

1. Create a Turnstile widget in Cloudflare Dashboard for `oudos-site.renmar.dev` (add `oudos.gombio2418.workers.dev` if also testing on the `.workers.dev` fallback).
2. Set the secret key via Wrangler (or in Cloudflare Dashboard under **Settings -> Variables and Secrets**):

```bash
cd site
npx wrangler secret put TURNSTILE_SECRET_KEY
```

3. Set non-secret vars in Cloudflare Dashboard (**Workers & Pages -> `oudos` -> Settings -> Variables and Secrets**) or `wrangler.jsonc` `vars`:

- `TURNSTILE_SITE_KEY` — Public Turnstile site key returned by `GET /api/config`
- `ALLOWED_HOSTNAME` — Optional expected hostname (set to `oudos-site.renmar.dev` for production) enforced against Turnstile's `siteverify` response (leave unset if testing on `oudos.gombio2418.workers.dev`)
- `CONTACT_WEBHOOK_URL` — Optional HTTPS webhook URL receiving verified JSON contact submissions
