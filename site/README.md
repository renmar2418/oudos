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

## Custom Domain & Environment Variables

Attach your custom domain in **Workers -> `oudos-site` -> Settings -> Domains & Routes -> Add -> Custom Domain**.

### Optional Turnstile + Contact Endpoint

The site works with zero secrets configured (downloads are never gated, and the contact section falls back to GitHub Issues). To enable the live `POST /api/contact` form:

1. Create a Turnstile widget in Cloudflare Dashboard for your domain.
2. Set the public site key as a Worker variable (or secret) and the secret key via Wrangler:

```bash
cd site
npx wrangler secret put TURNSTILE_SECRET_KEY
```

Set non-secret vars in Cloudflare Dashboard (**Workers -> Settings -> Variables and Secrets**) or `wrangler.jsonc` `vars`:

- `TURNSTILE_SITE_KEY` — Public Turnstile site key returned by `GET /api/config`
- `ALLOWED_HOSTNAME` — Optional expected hostname (e.g. `oudos.example.org`) enforced against Turnstile's `siteverify` response
- `CONTACT_WEBHOOK_URL` — Optional HTTPS webhook URL receiving verified JSON contact submissions
