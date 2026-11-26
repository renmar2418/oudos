const TURNSTILE_VERIFY_URL = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
const MAX_TOKEN_LENGTH = 2048;
const MAX_BODY_BYTES = 16384;
const ALLOWED_TOPICS = new Set([
  "usability-study",
  "bug-report",
  "self-hosting",
  "security-note",
]);

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: {
      "Content-Type": "application/json; charset=utf-8",
      "X-Content-Type-Options": "nosniff",
      "Cache-Control": "no-store",
      "Referrer-Policy": "strict-origin-when-cross-origin",
      "X-Frame-Options": "DENY",
    },
  });
}

function isValidEmail(email) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
}

async function handleConfig(env) {
  const siteKey = typeof env.TURNSTILE_SITE_KEY === "string"
    ? env.TURNSTILE_SITE_KEY.trim()
    : "";
  return jsonResponse({ turnstileSiteKey: siteKey });
}

async function handleContact(request, env) {
  const secretKey = typeof env.TURNSTILE_SECRET_KEY === "string"
    ? env.TURNSTILE_SECRET_KEY.trim()
    : "";

  if (!secretKey) {
    return jsonResponse(
      { ok: false, error: "Contact form verification is not configured on this deployment." },
      503
    );
  }

  const contentType = request.headers.get("Content-Type") || "";
  if (!contentType.toLowerCase().includes("application/json")) {
    return jsonResponse({ ok: false, error: "Expected application/json request body." }, 415);
  }

  const rawBody = await request.text();
  if (rawBody.length > MAX_BODY_BYTES) {
    return jsonResponse({ ok: false, error: "Request payload exceeds maximum allowed size." }, 413);
  }

  let body;
  try {
    body = JSON.parse(rawBody);
  } catch {
    return jsonResponse({ ok: false, error: "Invalid JSON payload." }, 400);
  }

  if (!body || typeof body !== "object" || Array.isArray(body)) {
    return jsonResponse({ ok: false, error: "Request body must be a JSON object." }, 400);
  }

  const name = typeof body.name === "string" ? body.name.trim() : "";
  const email = typeof body.email === "string" ? body.email.trim() : "";
  const topic = typeof body.topic === "string" ? body.topic.trim() : "";
  const message = typeof body.message === "string" ? body.message.trim() : "";
  const token = typeof body.turnstileToken === "string"
    ? body.turnstileToken.trim()
    : (typeof body["cf-turnstile-response"] === "string"
      ? body["cf-turnstile-response"].trim()
      : "");

  const fieldErrors = {};
  if (name.length < 1 || name.length > 120) {
    fieldErrors.name = "Enter a name between 1 and 120 characters.";
  }
  if (email.length < 3 || email.length > 254 || !isValidEmail(email)) {
    fieldErrors.email = "Enter a valid email address (max 254 characters).";
  }
  if (!ALLOWED_TOPICS.has(topic)) {
    fieldErrors.topic = "Select a valid topic.";
  }
  if (message.length < 10 || message.length > 2000) {
    fieldErrors.message = "Enter a message between 10 and 2,000 characters.";
  }
  if (token.length < 1 || token.length > MAX_TOKEN_LENGTH) {
    fieldErrors.turnstile = "Complete the Cloudflare Turnstile check before submitting.";
  }

  if (Object.keys(fieldErrors).length > 0) {
    return jsonResponse(
      { ok: false, error: "Please check the highlighted fields.", fieldErrors },
      400
    );
  }

  const verifyForm = new URLSearchParams();
  verifyForm.set("secret", secretKey);
  verifyForm.set("response", token);

  const remoteIp = request.headers.get("CF-Connecting-IP");
  if (remoteIp) {
    verifyForm.set("remoteip", remoteIp);
  }

  let verifyData;
  try {
    const verifyRes = await fetch(TURNSTILE_VERIFY_URL, {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: verifyForm.toString(),
    });
    if (!verifyRes.ok) {
      return jsonResponse({ ok: false, error: "Turnstile verification service error." }, 502);
    }
    verifyData = await verifyRes.json();
  } catch {
    return jsonResponse({ ok: false, error: "Unable to reach Turnstile verification." }, 502);
  }

  if (!verifyData || verifyData.success !== true) {
    return jsonResponse(
      {
        ok: false,
        error: "Verification check failed. Please retry the Turnstile widget.",
        fieldErrors: { turnstile: "Verification token rejected or expired." },
      },
      403
    );
  }

  const allowedHost = typeof env.ALLOWED_HOSTNAME === "string"
    ? env.ALLOWED_HOSTNAME.trim()
    : "";
  if (allowedHost && verifyData.hostname !== allowedHost) {
    return jsonResponse({ ok: false, error: "Verification hostname mismatch." }, 403);
  }

  const webhookUrl = typeof env.CONTACT_WEBHOOK_URL === "string"
    ? env.CONTACT_WEBHOOK_URL.trim()
    : "";
  if (webhookUrl) {
    try {
      const forwardRes = await fetch(webhookUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json; charset=utf-8" },
        body: JSON.stringify({
          source: "oudos-site",
          name,
          email,
          topic,
          message,
          hostname: verifyData.hostname || null,
          submittedAt: new Date().toISOString(),
        }),
      });
      if (!forwardRes.ok) {
        return jsonResponse({ ok: false, error: "Failed to deliver submission." }, 502);
      }
    } catch {
      return jsonResponse({ ok: false, error: "Delivery webhook unreachable." }, 502);
    }
  }

  return jsonResponse({
    ok: true,
    message: "Submission verified and recorded. Thank you for helping test Oudos.",
  });
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (url.pathname === "/api/config") {
      if (request.method !== "GET") {
        return jsonResponse({ ok: false, error: "Method not allowed." }, 405);
      }
      return handleConfig(env);
    }

    if (url.pathname === "/api/contact") {
      if (request.method !== "POST") {
        return jsonResponse({ ok: false, error: "Method not allowed." }, 405);
      }
      return handleContact(request, env);
    }

    if (url.pathname.startsWith("/api/")) {
      return jsonResponse({ ok: false, error: "Not found." }, 404);
    }

    return env.ASSETS.fetch(request);
  },
};
