(() => {
  "use strict";

  // Mobile nav toggle
  const navToggle = document.getElementById("nav-toggle");
  const siteNav = document.getElementById("site-nav");
  if (navToggle && siteNav) {
    navToggle.addEventListener("click", () => {
      const expanded = navToggle.getAttribute("aria-expanded") === "true";
      navToggle.setAttribute("aria-expanded", String(!expanded));
      siteNav.classList.toggle("is-open", !expanded);
    });

    siteNav.addEventListener("click", (event) => {
      if (event.target instanceof HTMLAnchorElement && window.innerWidth < 768) {
        navToggle.setAttribute("aria-expanded", "false");
        siteNav.classList.remove("is-open");
      }
    });
  }

  // Copy-to-clipboard buttons
  const copyButtons = document.querySelectorAll("[data-copy-target]");
  copyButtons.forEach((button) => {
    const defaultLabel = button.textContent.trim();
    button.addEventListener("click", async () => {
      const targetId = button.getAttribute("data-copy-target");
      const targetEl = targetId ? document.getElementById(targetId) : null;
      if (!targetEl) return;

      const text = targetEl.textContent.trim();
      try {
        await navigator.clipboard.writeText(text);
        button.textContent = "Copied";
        window.setTimeout(() => {
          button.textContent = defaultLabel;
        }, 1800);
      } catch {
        button.textContent = "Select & copy";
        window.setTimeout(() => {
          button.textContent = defaultLabel;
        }, 2000);
      }
    });
  });

  // Dynamic Turnstile config detection (/api/config)
  const fallbackBox = document.getElementById("contact-fallback");
  const contactForm = document.getElementById("contact-form");
  const turnstileContainer = document.getElementById("turnstile-container");
  const submitButton = document.getElementById("contact-submit");
  const errorSummary = document.getElementById("form-error-summary");
  const successSummary = document.getElementById("form-success-summary");

  if (!fallbackBox || !contactForm || !turnstileContainer || !submitButton) {
    return;
  }

  let turnstileWidgetId = null;
  let turnstileToken = "";

  function clearFieldErrors() {
    if (errorSummary) {
      errorSummary.hidden = true;
      errorSummary.textContent = "";
    }
    if (successSummary) {
      successSummary.hidden = true;
      successSummary.textContent = "";
    }

    const fields = ["name", "email", "topic", "message", "turnstile"];
    fields.forEach((field) => {
      const errEl = document.getElementById(`contact-${field}-error`);
      if (errEl) {
        errEl.hidden = true;
        errEl.textContent = "";
      }
      const inputEl = document.getElementById(`contact-${field}`);
      if (inputEl) {
        inputEl.removeAttribute("aria-invalid");
      }
    });
  }

  function showFormErrors(message, fieldErrors = {}) {
    Object.entries(fieldErrors).forEach(([field, errText]) => {
      const errEl = document.getElementById(`contact-${field}-error`);
      if (errEl) {
        errEl.textContent = errText;
        errEl.hidden = false;
      }
      const inputEl = document.getElementById(`contact-${field}`);
      if (inputEl) {
        inputEl.setAttribute("aria-invalid", "true");
      }
    });

    if (errorSummary) {
      errorSummary.textContent = message;
      errorSummary.hidden = false;
      errorSummary.focus();
    }
  }

  function loadTurnstileWidget(siteKey) {
    window.onOudosTurnstileLoad = () => {
      if (!window.turnstile) return;
      turnstileWidgetId = window.turnstile.render(turnstileContainer, {
        sitekey: siteKey,
        theme: "light",
        callback: (token) => {
          turnstileToken = token;
        },
        "expired-callback": () => {
          turnstileToken = "";
        },
        "error-callback": () => {
          turnstileToken = "";
        },
      });
    };

    const script = document.createElement("script");
    script.src = "https://challenges.cloudflare.com/turnstile/v0/api.js?onload=onOudosTurnstileLoad&render=explicit";
    script.async = true;
    script.defer = true;
    document.head.appendChild(script);
  }

  async function initContactConfig() {
    try {
      const res = await fetch("/api/config", {
        method: "GET",
        headers: { Accept: "application/json" },
      });
      if (!res.ok) return;
      const data = await res.json();
      const siteKey = data && typeof data.turnstileSiteKey === "string"
        ? data.turnstileSiteKey.trim()
        : "";
      if (!siteKey) return;

      fallbackBox.hidden = true;
      contactForm.hidden = false;
      loadTurnstileWidget(siteKey);
    } catch {
      // Static-file preview or unconfigured Worker keeps GitHub Issues fallback visible.
    }
  }

  contactForm.addEventListener("submit", async (event) => {
    event.preventDefault();
    clearFieldErrors();

    const name = document.getElementById("contact-name")?.value.trim() || "";
    const email = document.getElementById("contact-email")?.value.trim() || "";
    const topic = document.getElementById("contact-topic")?.value.trim() || "";
    const message = document.getElementById("contact-message")?.value.trim() || "";

    const localErrors = {};
    if (!name) localErrors.name = "Enter your name.";
    if (!email || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
      localErrors.email = "Enter a valid email address.";
    }
    if (message.length < 10) {
      localErrors.message = "Enter at least 10 characters.";
    }
    if (!turnstileToken) {
      localErrors.turnstile = "Complete the Cloudflare Turnstile check before submitting.";
    }

    if (Object.keys(localErrors).length > 0) {
      showFormErrors("Please check the highlighted fields before sending.", localErrors);
      return;
    }

    submitButton.disabled = true;
    submitButton.textContent = "Sending...";

    try {
      const res = await fetch("/api/contact", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          name,
          email,
          topic,
          message,
          turnstileToken,
        }),
      });
      const result = await res.json();
      if (!res.ok || !result.ok) {
        showFormErrors(
          result.error || "Unable to send submission.",
          result.fieldErrors || {}
        );
        if (window.turnstile && turnstileWidgetId !== null) {
          window.turnstile.reset(turnstileWidgetId);
          turnstileToken = "";
        }
        return;
      }

      contactForm.reset();
      if (window.turnstile && turnstileWidgetId !== null) {
        window.turnstile.reset(turnstileWidgetId);
        turnstileToken = "";
      }
      if (successSummary) {
        successSummary.textContent = result.message || "Thank you. Your message has been received.";
        successSummary.hidden = false;
      }
    } catch {
      showFormErrors("Network error while submitting. Please try again or use GitHub Issues.");
    } finally {
      submitButton.disabled = false;
      submitButton.textContent = "Send message";
    }
  });

  initContactConfig();
})();
