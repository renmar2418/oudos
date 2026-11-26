import React, { useMemo, useState } from "react";
import {
  Check,
  Copy,
  KeyRound,
  Laptop,
  Lock,
  QrCode,
  ShieldAlert,
  ShieldCheck,
  X,
} from "lucide-react";
import type { DeviceItem } from "../types";

interface AddDeviceWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onAddDevice: (device: DeviceItem) => void;
}

type WizardStep = 1 | 2 | 3;

function formatNineDigits(raw: string): string {
  const digits = raw.replace(/\D/g, "").slice(0, 9);
  return digits.replace(/(\d{3})(?=\d)/g, "$1 ").trim();
}

export const AddDeviceWizard: React.FC<AddDeviceWizardProps> = ({
  isOpen,
  onClose,
  onAddDevice,
}) => {
  const [step, setStep] = useState<WizardStep>(1);
  const [deviceIdInput, setDeviceIdInput] = useState("619 402 851");
  const [alias, setAlias] = useState("Support Laptop 02");
  const [ownerName, setOwnerName] = useState("Jordan");
  const [osName, setOsName] = useState("Windows 11 Pro");
  const [enrolledServer, setEnrolledServer] = useState(true);
  const [serverDomain, setServerDomain] = useState("support.example.org");
  const [pubKey, setPubKey] = useState(
    "ed25519:7c3a91e402b856d1c840f91a23b4c5d6e7f8091a2b3c"
  );
  const [copiedCode, setCopiedCode] = useState(false);
  const [validationError, setValidationError] = useState<string | null>(null);

  const pairingCode = "OUD-8492-K7M3";
  const pairingLink = `https://${
    enrolledServer ? serverDomain.trim() || "support.example.org" : "local.oudos"
  }/pair/${pairingCode}`;

  const compactDigits = useMemo(
    () => deviceIdInput.replace(/\D/g, ""),
    [deviceIdInput]
  );

  if (!isOpen) return null;

  const handleCopyCode = async () => {
    try {
      await navigator.clipboard?.writeText(pairingCode);
    } catch {
      // Clipboard API may be unavailable in non-HTTPS test sandboxes.
    }
    setCopiedCode(true);
  };

  const handleAdvanceToConfirm = (e: React.FormEvent) => {
    e.preventDefault();
    if (compactDigits.length !== 9) {
      setValidationError("Enter a valid 9-digit Device ID.");
      return;
    }
    if (!alias.trim() || !ownerName.trim()) {
      setValidationError("Enter both a device name and an owner name.");
      return;
    }
    if (!pubKey.startsWith("ed25519:") || pubKey.trim().length < 16) {
      setValidationError("Enter a valid Ed25519 host identity key (ed25519:...).");
      return;
    }
    setValidationError(null);
    setStep(3);
  };

  const handleConfirmPin = () => {
    const formattedId = formatNineDigits(compactDigits);
    const effectiveDomain = enrolledServer
      ? serverDomain.trim() || "support.example.org"
      : "Personal / Unverified";

    onAddDevice({
      deviceId: formattedId,
      alias: alias.trim(),
      ownerName: ownerName.trim(),
      osName,
      presence: "online",
      serverDomain: effectiveDomain,
      enrolledServer,
      lastSeenUtc: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
      pinnedPubkey: pubKey.trim(),
      accessMode: "asks_first",
    });
    setStep(1);
    onClose();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-[#111618]/60 p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="add-device-wizard-title"
    >
      <div className="max-h-[calc(100vh-2rem)] w-full max-w-xl overflow-y-auto border border-[#DADDD8] bg-white shadow-xl">
        <div className="flex items-center justify-between gap-4 border-b border-[#DADDD8] px-6 py-4">
          <div>
            <h2
              id="add-device-wizard-title"
              className="text-lg font-semibold text-[#111618]"
            >
              Pair a new host device
            </h2>
            <p className="text-xs text-[#525D63]">Step {step} of 3</p>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close Add Device wizard"
            className="rounded-md border border-[#DADDD8] p-1.5 text-[#525D63] hover:bg-[#F5F6F4] hover:text-[#111618] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52]"
          >
            <X className="h-4 w-4" aria-hidden="true" />
          </button>
        </div>

        <ol
          className="grid grid-cols-3 border-b border-[#DADDD8] bg-[#F5F6F4] text-xs divide-x divide-[#DADDD8]"
          aria-label="Pairing steps"
        >
          <li
            className={`border-b-2 px-3 py-2.5 font-medium ${
              step === 1
                ? "border-[#0F5C52] bg-white font-semibold text-[#111618]"
                : "border-transparent text-[#525D63]"
            }`}
          >
            1. Install &amp; code
          </li>
          <li
            className={`border-b-2 px-3 py-2.5 font-medium ${
              step === 2
                ? "border-[#0F5C52] bg-white font-semibold text-[#111618]"
                : "border-transparent text-[#525D63]"
            }`}
          >
            2. Identity &amp; mode
          </li>
          <li
            className={`border-b-2 px-3 py-2.5 font-medium ${
              step === 3
                ? "border-[#0F5C52] bg-white font-semibold text-[#111618]"
                : "border-transparent text-[#525D63]"
            }`}
          >
            3. Pin key
          </li>
        </ol>

        {step === 1 && (
          <div className="divide-y divide-[#DADDD8]">
            <div className="space-y-4 p-6">
              <p className="text-sm text-[#525D63]">
                Install the Oudos Windows host agent on the target PC, then enter this single-use pairing code or scan the pairing link.
              </p>

              <div className="grid gap-4 border border-[#DADDD8] bg-[#F5F6F4] p-4 sm:grid-cols-[1fr_auto] sm:items-center">
                <div>
                  <p className="text-xs font-medium text-[#525D63]">
                    Single-use pairing code
                  </p>
                  <p className="mt-1 font-mono-code text-2xl font-semibold text-[#111618]">
                    {pairingCode}
                  </p>
                  <p className="mt-1 break-all font-mono-code text-xs text-[#525D63]">
                    {pairingLink}
                  </p>
                  <button
                    type="button"
                    onClick={handleCopyCode}
                    className="mt-3 inline-flex items-center gap-1.5 rounded-md border border-[#DADDD8] bg-white px-3 py-1.5 text-xs font-semibold text-[#111618] hover:bg-[#E6F2F0] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52]"
                  >
                    {copiedCode ? (
                      <>
                        <Check className="h-3.5 w-3.5 text-[#083630]" aria-hidden="true" />
                        Copied pairing code
                      </>
                    ) : (
                      <>
                        <Copy className="h-3.5 w-3.5 text-[#0F5C52]" aria-hidden="true" />
                        Copy pairing code
                      </>
                    )}
                  </button>
                </div>

                <div
                  className="flex flex-col items-center justify-center border border-[#DADDD8] bg-white p-3"
                  role="img"
                  aria-label={`QR pairing link for ${pairingCode}`}
                >
                  <QrCode className="h-16 w-16 text-[#111618]" aria-hidden="true" />
                  <span className="mt-1 text-[11px] font-medium text-[#525D63]">
                    Scan to enroll
                  </span>
                </div>
              </div>

              <p className="border-l-2 border-[#0F5C52] pl-3 text-xs text-[#525D63]">
                Server enrollment pins your control plane&apos;s Ed25519 public key on the host before any prompt can be delivered.
              </p>
            </div>

            <div className="flex justify-end gap-3 bg-[#F5F6F4] px-6 py-3.5">
              <button
                type="button"
                onClick={onClose}
                className="rounded-md border border-[#DADDD8] bg-white px-4 py-2 text-sm font-medium text-[#111618] hover:bg-[#F5F6F4]"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={() => setStep(2)}
                className="rounded-md bg-[#0F5C52] px-4 py-2 text-sm font-semibold text-white hover:bg-[#0B473F] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52]"
              >
                Continue to device details
              </button>
            </div>
          </div>
        )}

        {step === 2 && (
          <form onSubmit={handleAdvanceToConfirm} className="divide-y divide-[#DADDD8]">
            <div className="space-y-4 p-6">
              <div className="grid gap-4 sm:grid-cols-2">
                <div>
                  <label
                    htmlFor="wizard-device-id"
                    className="block text-xs font-semibold text-[#111618]"
                  >
                    9-digit Device ID
                  </label>
                  <input
                    id="wizard-device-id"
                    type="text"
                    value={deviceIdInput}
                    onChange={(e) => setDeviceIdInput(formatNineDigits(e.target.value))}
                    className="mt-1 w-full rounded-md border border-[#DADDD8] bg-white px-3 py-2 font-mono-code text-sm text-[#111618] focus:border-[#0F5C52] focus:outline-none focus:ring-2 focus:ring-[#0F5C52]/25"
                  />
                </div>

                <div>
                  <label
                    htmlFor="wizard-device-alias"
                    className="block text-xs font-semibold text-[#111618]"
                  >
                    Device name
                  </label>
                  <input
                    id="wizard-device-alias"
                    type="text"
                    value={alias}
                    onChange={(e) => setAlias(e.target.value)}
                    className="mt-1 w-full rounded-md border border-[#DADDD8] bg-white px-3 py-2 text-sm text-[#111618] focus:border-[#0F5C52] focus:outline-none focus:ring-2 focus:ring-[#0F5C52]/25"
                  />
                </div>

                <div>
                  <label
                    htmlFor="wizard-owner-name"
                    className="block text-xs font-semibold text-[#111618]"
                  >
                    Device owner
                  </label>
                  <input
                    id="wizard-owner-name"
                    type="text"
                    value={ownerName}
                    onChange={(e) => setOwnerName(e.target.value)}
                    className="mt-1 w-full rounded-md border border-[#DADDD8] bg-white px-3 py-2 text-sm text-[#111618] focus:border-[#0F5C52] focus:outline-none focus:ring-2 focus:ring-[#0F5C52]/25"
                  />
                </div>

                <div>
                  <label
                    htmlFor="wizard-os-name"
                    className="block text-xs font-semibold text-[#111618]"
                  >
                    Operating system
                  </label>
                  <select
                    id="wizard-os-name"
                    value={osName}
                    onChange={(e) => setOsName(e.target.value)}
                    className="mt-1 w-full rounded-md border border-[#DADDD8] bg-white px-3 py-2 text-sm text-[#111618] focus:border-[#0F5C52] focus:outline-none focus:ring-2 focus:ring-[#0F5C52]/25"
                  >
                    <option value="Windows 11 Pro">Windows 11 Pro</option>
                    <option value="Windows 11 Home">Windows 11 Home</option>
                    <option value="Windows 10 Pro">Windows 10 Pro</option>
                  </select>
                </div>
              </div>

              <div>
                <label
                  htmlFor="wizard-pubkey"
                  className="block text-xs font-semibold text-[#111618]"
                >
                  Host Ed25519 identity key (pinned on first use)
                </label>
                <input
                  id="wizard-pubkey"
                  type="text"
                  value={pubKey}
                  onChange={(e) => setPubKey(e.target.value)}
                  className="mt-1 w-full rounded-md border border-[#DADDD8] bg-white px-3 py-2 font-mono-code text-xs text-[#111618] focus:border-[#0F5C52] focus:outline-none focus:ring-2 focus:ring-[#0F5C52]/25"
                />
              </div>

              <div className="flex flex-col gap-3 border border-[#DADDD8] bg-[#F5F6F4] p-3.5 sm:flex-row sm:items-center sm:justify-between">
                <div className="flex items-center gap-2 text-xs text-[#111618]">
                  <Lock className="h-4 w-4 text-[#0F5C52]" aria-hidden="true" />
                  <span>
                    Access mode: <strong>Asks-First</strong> (Unattended mode locked OFF in v0.1)
                  </span>
                </div>
                <label className="inline-flex items-center gap-2 text-xs font-medium text-[#111618]">
                  <input
                    type="checkbox"
                    checked={enrolledServer}
                    onChange={(e) => setEnrolledServer(e.target.checked)}
                    className="h-4 w-4 accent-[#0F5C52]"
                  />
                  Enrolled server ({serverDomain})
                </label>
              </div>

              {enrolledServer && (
                <div>
                  <label
                    htmlFor="wizard-server-domain"
                    className="block text-xs font-semibold text-[#111618]"
                  >
                    Pinned server domain
                  </label>
                  <input
                    id="wizard-server-domain"
                    type="text"
                    value={serverDomain}
                    onChange={(e) => setServerDomain(e.target.value)}
                    className="mt-1 w-full rounded-md border border-[#DADDD8] bg-white px-3 py-2 font-mono-code text-sm text-[#111618] focus:border-[#0F5C52] focus:outline-none focus:ring-2 focus:ring-[#0F5C52]/25"
                  />
                </div>
              )}

              {validationError && (
                <p
                  role="alert"
                  className="border border-[#B42318]/30 bg-[#FEE4E2] px-3 py-2 text-xs font-medium text-[#912018]"
                >
                  {validationError}
                </p>
              )}
            </div>

            <div className="flex justify-between gap-3 bg-[#F5F6F4] px-6 py-3.5">
              <button
                type="button"
                onClick={() => setStep(1)}
                className="rounded-md border border-[#DADDD8] bg-white px-4 py-2 text-sm font-medium text-[#111618] hover:bg-[#F5F6F4]"
              >
                Back
              </button>
              <button
                type="submit"
                className="rounded-md bg-[#0F5C52] px-4 py-2 text-sm font-semibold text-white hover:bg-[#0B473F] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52]"
              >
                Review &amp; pin key
              </button>
            </div>
          </form>
        )}

        {step === 3 && (
          <div className="divide-y divide-[#DADDD8]">
            <div className="space-y-4 p-6">
              <div className="divide-y divide-[#DADDD8] border border-[#DADDD8] text-sm">
                <div className="flex items-center justify-between bg-[#F5F6F4] px-4 py-3">
                  <span className="inline-flex items-center gap-2 font-semibold text-[#111618]">
                    <Laptop className="h-4 w-4 text-[#0F5C52]" aria-hidden="true" />
                    {alias}
                  </span>
                  <span className="font-mono-code text-xs font-semibold text-[#111618]">
                    {formatNineDigits(compactDigits)}
                  </span>
                </div>

                <div className="flex items-center justify-between px-4 py-2.5 text-xs">
                  <span className="text-[#525D63]">Owner &amp; OS</span>
                  <span className="font-medium text-[#111618]">
                    {ownerName} · {osName} · Asks-First
                  </span>
                </div>

                <div className="flex items-center justify-between gap-2 px-4 py-2.5 text-xs">
                  <span className="text-[#525D63]">Server trust</span>
                  {enrolledServer ? (
                    <span className="inline-flex items-center gap-1 font-mono-code font-semibold text-[#0F5C52]">
                      <ShieldCheck className="h-3.5 w-3.5" aria-hidden="true" />
                      {serverDomain.trim() || "support.example.org"}
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1 font-mono-code font-semibold text-[#B54708]">
                      <ShieldAlert className="h-3.5 w-3.5" aria-hidden="true" />
                      Personal / Unverified
                    </span>
                  )}
                </div>

                <div className="px-4 py-2.5">
                  <p className="inline-flex items-center gap-1.5 text-xs font-semibold text-[#111618]">
                    <KeyRound className="h-3.5 w-3.5 text-[#0F5C52]" aria-hidden="true" />
                    Pinned Ed25519 Fingerprint
                  </p>
                  <p className="mt-1 break-all font-mono-code text-xs text-[#525D63]">
                    {pubKey}
                  </p>
                </div>
              </div>

              <p className="text-xs text-[#525D63]">
                Future connections will verify this saved key. If the device key changes later, Oudos halts immediately and alerts you.
              </p>
            </div>

            <div className="flex justify-between gap-3 bg-[#F5F6F4] px-6 py-3.5">
              <button
                type="button"
                onClick={() => setStep(2)}
                className="rounded-md border border-[#DADDD8] bg-white px-4 py-2 text-sm font-medium text-[#111618] hover:bg-[#F5F6F4]"
              >
                Back
              </button>
              <button
                type="button"
                onClick={handleConfirmPin}
                className="rounded-md bg-[#0F5C52] px-4 py-2 text-sm font-semibold text-white hover:bg-[#0B473F] focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52]"
              >
                Pin key &amp; add device
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
