import React, { useCallback, useEffect, useMemo, useState } from "react";
import { FacingBracketsIcon } from "./components/FacingBracketsIcon";
import { AddDeviceWizard } from "./components/AddDeviceWizard";
import {
  CodeMismatchCard,
  ControlPlaneOfflineCard,
  DeviceKeyChangedAlert,
  OwnerDeniedCard,
  RequestExpiredCard,
  ScreenUnavailableCard,
  SessionEndedSummary,
} from "./components/EdgeStatePanels";
import {
  DEFAULT_VIEW_ONLY_PERMISSIONS,
  INITIAL_POLICY,
  INITIAL_SESSIONS,
  SAMPLE_AUDIT_ENTRIES,
  SAMPLE_DEVICES,
} from "./data/sampleData";
import { ActiveSessionScreen } from "./screens/ActiveSessionScreen";
import { AuditLogScreen } from "./screens/AuditLogScreen";
import { DevicesScreen } from "./screens/DevicesScreen";
import { PoliciesScreen } from "./screens/PoliciesScreen";
import { RequestingScreen } from "./screens/RequestingScreen";
import { SessionsScreen } from "./screens/SessionsScreen";
import {
  createBrowserSessionTransport,
  type IncomingSignalEvent,
  type SessionTransport,
} from "./services/sessionTransport";
import type {
  AuditEntryItem,
  CoreTab,
  DeviceItem,
  GrantedPermissions,
  PolicyState,
  RequestFlowOutcome,
  ScreenUnavailableReason,
  SessionEndReason,
  SessionRecord,
} from "./types";

const NAV_ITEMS: { id: CoreTab; label: string }[] = [
  { id: "devices", label: "Devices" },
  { id: "sessions", label: "Sessions" },
  { id: "requesting", label: "Requesting" },
  { id: "session", label: "Active Session" },
  { id: "policies", label: "Policies" },
  { id: "audit", label: "Audit Log" },
];

function nextSessionId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `sess-${Date.now()}-${Math.floor(Math.random() * 10000)}`;
}

function nowUtcIso(): string {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}

function deriveDeterministicSas(seed: string): string {
  let acc = 2166136261;
  for (let i = 0; i < seed.length; i += 1) {
    acc ^= seed.charCodeAt(i);
    acc = Math.imul(acc, 16777619);
  }
  const sixDigits = String((acc >>> 0) % 1_000_000).padStart(6, "0");
  return `${sixDigits.slice(0, 3)} ${sixDigits.slice(3, 6)}`;
}

export interface AppProps {
  initialTab?: CoreTab;
  transport?: SessionTransport;
  initialDevices?: DeviceItem[];
  initialSessions?: SessionRecord[];
  initialPolicy?: PolicyState;
}

export const App: React.FC<AppProps> = ({
  initialTab = "devices",
  transport: injectedTransport,
  initialDevices = SAMPLE_DEVICES,
  initialSessions = INITIAL_SESSIONS,
  initialPolicy = INITIAL_POLICY,
}) => {
  const defaultTransport = useMemo(
    () => injectedTransport ?? createBrowserSessionTransport(),
    [injectedTransport]
  );

  const [activeTab, setActiveTab] = useState<CoreTab>(initialTab);
  const [devices, setDevices] = useState<DeviceItem[]>(initialDevices);
  const [selectedDeviceId, setSelectedDeviceId] = useState<string>(
    initialDevices[0]?.deviceId ?? ""
  );
  const [policy, setPolicy] = useState<PolicyState>(initialPolicy);
  const [sessions, setSessions] = useState<SessionRecord[]>(initialSessions);
  const [auditEntries, setAuditEntries] =
    useState<AuditEntryItem[]>(SAMPLE_AUDIT_ENTRIES);

  const [isAddDeviceOpen, setIsAddDeviceOpen] = useState(false);
  const [controlPlaneOffline, setControlPlaneOffline] = useState(false);
  const [controlPlaneReason, setControlPlaneReason] = useState(
    "Signaling WebSocket disconnected"
  );
  const [requestedPermissions, setRequestedPermissions] =
    useState<GrantedPermissions>({ ...DEFAULT_VIEW_ONLY_PERMISSIONS });
  const [activeRequestSessionId, setActiveRequestSessionId] = useState<string>(
    "019363f2-9999-7000-8000-000000000001"
  );
  const [sasCode, setSasCode] = useState<string>("739 204");
  const [requestOutcome, setRequestOutcome] = useState<RequestFlowOutcome>({
    kind: "awaiting_approval",
    isJoinRequest: false,
  });
  const [activeSessionId, setActiveSessionId] = useState<string>(
    initialSessions.find((s) => s.status === "active")?.sessionId ??
      initialSessions[0]?.sessionId ??
      ""
  );
  const [endedSummarySession, setEndedSummarySession] =
    useState<SessionRecord | null>(null);

  const selectedDevice = useMemo(
    () =>
      devices.find((d) => d.deviceId === selectedDeviceId) ??
      devices[0] ??
      SAMPLE_DEVICES[0]!,
    [devices, selectedDeviceId]
  );

  const activeSession = useMemo(() => {
    const found =
      sessions.find((s) => s.sessionId === activeSessionId) ??
      sessions.find((s) => s.status === "active");
    if (found) return found;
    return {
      sessionId: activeRequestSessionId,
      deviceId: selectedDevice.deviceId,
      deviceAlias: selectedDevice.alias,
      ownerName: selectedDevice.ownerName,
      requester: "alex@support.example.org",
      approver: selectedDevice.ownerName,
      serverDomain: selectedDevice.serverDomain,
      enrolledServer: selectedDevice.enrolledServer,
      status: "active" as const,
      startedAtUtc: nowUtcIso(),
      durationLabel: "0m 01s",
      permissions: { ...DEFAULT_VIEW_ONLY_PERMISSIONS },
      transportMode: "direct_p2p" as const,
      latencyMs: 38,
    };
  }, [activeRequestSessionId, activeSessionId, selectedDevice, sessions]);

  const appendAuditEntry = useCallback(
    (device: DeviceItem, sessionId: string, action: string) => {
      setAuditEntries((prev) => {
        const last = prev[prev.length - 1];
        const nextSeq = last ? last.sequence + 1 : 0;
        const prevHash =
          last?.entryHash ??
          "0000000000000000000000000000000000000000000000000000000000000000";
        const pseudoHash = `${nextSeq.toString(16).padStart(4, "0")}${prevHash.slice(4, 64)}`;
        const entry: AuditEntryItem = {
          sequence: nextSeq,
          eventId: nextSessionId(),
          timestampUtc: nowUtcIso(),
          sessionId,
          deviceAlias: device.alias,
          deviceId: device.deviceId,
          actor: "alex@support.example.org",
          serverDomain: device.serverDomain,
          action,
          prevHash,
          entryHash: pseudoHash,
          hostSignature: "ed25519_sig:host_signed_event",
        };
        return [...prev, entry];
      });
    },
    []
  );

  const finalizeSessionEnd = useCallback(
    (sessionId: string, reason: SessionEndReason) => {
      const target = sessions.find((s) => s.sessionId === sessionId) ?? activeSession;
      const endedRecord: SessionRecord = {
        ...target,
        status: "ended",
        endedAtUtc: nowUtcIso(),
        endReason: reason,
        hostOfflineMidSession: false,
        auditSequence: auditEntries.length,
        auditEntryHash:
          auditEntries[auditEntries.length - 1]?.entryHash ??
          "c19d83e472f60514b89a2c3d4e5f6a7b8c9d0e1f2a3b4c5d",
      };
      setSessions((prev) => {
        const exists = prev.some((s) => s.sessionId === endedRecord.sessionId);
        if (!exists) return [endedRecord, ...prev];
        return prev.map((s) =>
          s.sessionId === endedRecord.sessionId ? endedRecord : s
        );
      });
      const dev =
        devices.find((d) => d.deviceId === endedRecord.deviceId) ?? selectedDevice;
      appendAuditEntry(
        dev,
        endedRecord.sessionId,
        `SessionRevoked (${reason})`
      );
      setEndedSummarySession(endedRecord);
      setActiveTab("session");
    },
    [activeSession, appendAuditEntry, auditEntries, devices, selectedDevice, sessions]
  );

  const handleSignalEventRef = React.useRef<(event: IncomingSignalEvent) => void>(() => {});

  handleSignalEventRef.current = (event: IncomingSignalEvent) => {
    switch (event.type) {
      case "grant_activated": {
        setControlPlaneOffline(false);
        const sessionDevice = selectedDevice;
        const nextSession: SessionRecord = {
          sessionId: event.sessionId || activeRequestSessionId,
          deviceId: sessionDevice.deviceId,
          deviceAlias: sessionDevice.alias,
          ownerName: sessionDevice.ownerName,
          requester: "alex@support.example.org",
          approver: event.approver || sessionDevice.ownerName,
          serverDomain: sessionDevice.serverDomain,
          enrolledServer: sessionDevice.enrolledServer,
          status: "active",
          startedAtUtc: nowUtcIso(),
          durationLabel: "0m 05s",
          permissions: {
            viewScreen: true,
            mouseKeyboard:
              event.permissions.mouseKeyboard && policy.allowMouseKeyboard.value,
            clipboardSync:
              event.permissions.clipboardSync && policy.allowClipboardSync.value,
            fileTransfer:
              event.permissions.fileTransfer && policy.allowFileTransfer.value,
            blankScreen: false,
          },
          transportMode: event.transportMode ?? "direct_p2p",
          latencyMs: event.latencyMs ?? 38,
        };
        setSessions((prev) => [
          nextSession,
          ...prev.filter(
            (s) => s.sessionId !== nextSession.sessionId && s.status !== "pending"
          ),
        ]);
        setEndedSummarySession(null);
        setActiveSessionId(nextSession.sessionId);
        appendAuditEntry(
          sessionDevice,
          nextSession.sessionId,
          "GrantIssued (Host signed SessionGrant)"
        );
        setActiveTab("session");
        break;
      }
      case "owner_denied": {
        setRequestOutcome({
          kind: "owner_denied",
          deniedAtUtc: nowUtcIso(),
          streakCount: event.streakCount ?? 1,
        });
        appendAuditEntry(
          selectedDevice,
          event.sessionId || activeRequestSessionId,
          "ConsentDenied (Owner declined)"
        );
        setActiveTab("requesting");
        break;
      }
      case "request_expired": {
        setRequestOutcome({
          kind: "request_expired",
          expiredAtUtc: nowUtcIso(),
        });
        appendAuditEntry(
          selectedDevice,
          event.sessionId || activeRequestSessionId,
          "ConsentDenied (2-minute TTL expired)"
        );
        setActiveTab("requesting");
        break;
      }
      case "code_mismatch": {
        setRequestOutcome({
          kind: "code_mismatch",
          ...(event.reportedCode !== undefined
            ? { reportedCode: event.reportedCode }
            : {}),
          streakCount: event.streakCount ?? 1,
        });
        appendAuditEntry(
          selectedDevice,
          event.sessionId || activeRequestSessionId,
          "ConsentDenied (6-digit SAS mismatch)"
        );
        setActiveTab("requesting");
        break;
      }
      case "host_key_changed": {
        setRequestOutcome({
          kind: "device_key_changed",
          expectedPubkey: event.expectedPubkey || selectedDevice.pinnedPubkey,
          presentedPubkey: event.presentedPubkey,
        });
        appendAuditEntry(
          selectedDevice,
          event.sessionId || activeRequestSessionId,
          "BlockedSecurityAlert (Device key changed)"
        );
        setActiveTab("requesting");
        break;
      }
      case "screen_unavailable": {
        setDevices((prev) =>
          prev.map((d) =>
            d.deviceId === selectedDevice.deviceId
              ? { ...d, screenUnavailable: event.reason }
              : d
          )
        );
        setRequestOutcome({
          kind: "screen_unavailable",
          reason: event.reason,
        });
        setActiveTab("requesting");
        break;
      }
      case "host_offline": {
        setSessions((prev) =>
          prev.map((s) =>
            s.sessionId === (event.sessionId || activeSessionId)
              ? {
                  ...s,
                  hostOfflineMidSession: true,
                  hostOfflineSinceUtc: nowUtcIso(),
                }
              : s
          )
        );
        setActiveTab("session");
        break;
      }
      case "control_plane_offline": {
        setControlPlaneOffline(true);
        setControlPlaneReason(event.reason);
        if (activeTab === "requesting") {
          setRequestOutcome({
            kind: "control_plane_offline",
            reason: event.reason,
          });
        }
        break;
      }
      case "control_plane_online": {
        setControlPlaneOffline(false);
        if (requestOutcome.kind === "control_plane_offline") {
          setRequestOutcome({ kind: "awaiting_approval", isJoinRequest: false });
        }
        break;
      }
      case "session_ended": {
        finalizeSessionEnd(event.sessionId || activeSessionId, event.reason);
        break;
      }
    }
  };

  useEffect(() => {
    return defaultTransport.subscribe((event) => {
      handleSignalEventRef.current(event);
    });
  }, [defaultTransport]);

  const startConnectionFlow = (device: DeviceItem) => {
    setSelectedDeviceId(device.deviceId);
    setEndedSummarySession(null);

    if (controlPlaneOffline) {
      setRequestOutcome({
        kind: "control_plane_offline",
        reason: controlPlaneReason,
      });
      setActiveTab("requesting");
      return;
    }

    if (
      device.presentedPubkey &&
      device.presentedPubkey !== device.pinnedPubkey
    ) {
      setRequestOutcome({
        kind: "device_key_changed",
        expectedPubkey: device.pinnedPubkey,
        presentedPubkey: device.presentedPubkey,
      });
      appendAuditEntry(
        device,
        nextSessionId(),
        "BlockedSecurityAlert (Device key changed)"
      );
      setActiveTab("requesting");
      return;
    }

    if (device.screenUnavailable) {
      setRequestOutcome({
        kind: "screen_unavailable",
        reason: device.screenUnavailable,
      });
      setActiveTab("requesting");
      return;
    }

    const sessionId = nextSessionId();
    const nextSas =
      device.deviceId === SAMPLE_DEVICES[0]?.deviceId
        ? "739 204"
        : deriveDeterministicSas(`${device.deviceId}:${sessionId}`);
    setActiveRequestSessionId(sessionId);
    setSasCode(nextSas);
    setRequestedPermissions({ ...DEFAULT_VIEW_ONLY_PERMISSIONS });
    setRequestOutcome({
      kind: "awaiting_approval",
      isJoinRequest: device.presence === "in_session",
    });

    setSessions((prev) => [
      {
        sessionId,
        deviceId: device.deviceId,
        deviceAlias: device.alias,
        ownerName: device.ownerName,
        requester: "alex@support.example.org",
        approver: device.ownerName,
        serverDomain: device.serverDomain,
        enrolledServer: device.enrolledServer,
        status: "pending",
        startedAtUtc: nowUtcIso(),
        durationLabel: "Pending",
        permissions: { ...DEFAULT_VIEW_ONLY_PERMISSIONS },
        transportMode: "direct_p2p",
        latencyMs: 38,
      },
      ...prev.filter((s) => s.status !== "pending"),
    ]);

    appendAuditEntry(device, sessionId, "SessionRequested");
    defaultTransport.send({
      type: "session_init",
      sessionId,
      targetDeviceId: device.deviceId,
      sasCode: nextSas,
      requestedPermissions: { ...DEFAULT_VIEW_ONLY_PERMISSIONS },
    });
    setActiveTab("requesting");
  };

  const handleAddDevice = (newDevice: DeviceItem) => {
    setDevices((prev) => [newDevice, ...prev]);
    appendAuditEntry(
      newDevice,
      nextSessionId(),
      `DeviceEnrolled (${newDevice.serverDomain})`
    );
  };

  const handleToggleNotify = (deviceId: string) => {
    setDevices((prev) =>
      prev.map((d) =>
        d.deviceId === deviceId
          ? { ...d, notifyWhenOnline: !d.notifyWhenOnline }
          : d
      )
    );
  };

  const handleScreenRetry = () => {
    defaultTransport.send({
      type: "recheck_screen",
      targetDeviceId: selectedDevice.deviceId,
    });
    const clearedDevice: DeviceItem = {
      ...selectedDevice,
      screenUnavailable: null,
    };
    setDevices((prev) =>
      prev.map((d) =>
        d.deviceId === selectedDevice.deviceId ? clearedDevice : d
      )
    );
    startConnectionFlow(clearedDevice);
  };

  const handleAcceptRepinnedKey = (newPubkey: string) => {
    const updatedDevice: DeviceItem = {
      ...selectedDevice,
      pinnedPubkey: newPubkey,
      presentedPubkey: newPubkey,
    };
    setDevices((prev) =>
      prev.map((d) =>
        d.deviceId === selectedDevice.deviceId ? updatedDevice : d
      )
    );
    appendAuditEntry(
      updatedDevice,
      activeRequestSessionId,
      "HostKeyRepinned (Owner-verified recovery)"
    );
    startConnectionFlow(updatedDevice);
  };

  const handleRequestExpired = useCallback(() => {
    if (requestOutcome.kind === "request_expired") return;
    setRequestOutcome({
      kind: "request_expired",
      expiredAtUtc: nowUtcIso(),
    });
    setSessions((prev) => prev.filter((s) => s.status !== "pending"));
    appendAuditEntry(
      selectedDevice,
      activeRequestSessionId,
      "ConsentDenied (2-minute TTL expired)"
    );
  }, [activeRequestSessionId, appendAuditEntry, requestOutcome.kind, selectedDevice]);

  const handleCodeMismatchAbort = () => {
    defaultTransport.send({
      type: "code_mismatch_abort",
      sessionId: activeRequestSessionId,
      targetDeviceId: selectedDevice.deviceId,
    });
    setRequestOutcome({
      kind: "code_mismatch",
      streakCount: 1,
    });
    setSessions((prev) => prev.filter((s) => s.status !== "pending"));
    appendAuditEntry(
      selectedDevice,
      activeRequestSessionId,
      "ConsentDenied (6-digit SAS mismatch)"
    );
  };

  const handleCancelRequest = () => {
    defaultTransport.send({
      type: "cancel_request",
      sessionId: activeRequestSessionId,
      targetDeviceId: selectedDevice.deviceId,
    });
    setSessions((prev) => prev.filter((s) => s.status !== "pending"));
    setActiveTab("devices");
  };

  const isLiveSessionView = activeTab === "session" && !endedSummarySession;
  const iconState =
    requestOutcome.kind === "device_key_changed" ||
    requestOutcome.kind === "code_mismatch"
      ? "alert"
      : isLiveSessionView
        ? "in_session"
        : "idle";

  return (
    <div
      className={`flex min-h-screen flex-col ${
        isLiveSessionView
          ? "bg-[#0E1214] text-white"
          : "bg-[#F5F6F4] text-[#111618]"
      }`}
    >
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-50 focus:rounded-md focus:bg-[#0F5C52] focus:px-4 focus:py-2 focus:text-sm focus:font-semibold focus:text-white"
      >
        Skip to main content
      </a>

      <header className="border-b border-[#DADDD8] bg-white text-[#111618]">
        <div className="mx-auto flex max-w-6xl flex-col sm:flex-row sm:items-center sm:justify-between sm:gap-x-6 sm:px-6">
          <div className="flex flex-col sm:flex-row sm:items-center sm:gap-5">
            <div className="flex items-center justify-between px-4 py-2.5 sm:px-0 sm:py-0">
              <div className="flex items-center gap-2">
                <span className={iconState === "alert" ? "text-[#B42318]" : "text-[#0F5C52]"}>
                  <FacingBracketsIcon state={iconState} className="h-6 w-6" label="Oudos" />
                </span>
                <span className="font-display text-lg font-semibold tracking-[-0.02em] text-[#111618]">
                  Oudos
                </span>
              </div>
              <span className="font-mono-code text-[11px] font-medium text-[#0F5C52] sm:hidden">
                support.example.org
              </span>
            </div>

            <nav
              aria-label="Primary navigation"
              className="flex items-center gap-1 overflow-x-auto border-t border-[#DADDD8]/70 px-4 sm:border-t-0 sm:px-0"
            >
              {NAV_ITEMS.map((item) => {
                const current = activeTab === item.id;
                return (
                  <button
                    key={item.id}
                    type="button"
                    aria-current={current ? "page" : undefined}
                    onClick={() => {
                      if (item.id === "session") setEndedSummarySession(null);
                      setActiveTab(item.id);
                    }}
                    className={`border-b-2 px-2 py-2.5 text-xs whitespace-nowrap transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-[#0F5C52] sm:px-2.5 sm:py-3 sm:text-sm ${
                      current
                        ? "border-[#0F5C52] font-semibold text-[#111618]"
                        : "border-transparent font-medium text-[#525D63] hover:border-[#DADDD8] hover:text-[#111618]"
                    }`}
                  >
                    {item.label}
                  </button>
                );
              })}
            </nav>
          </div>

          <div
            aria-label="Account and server identity"
            className="hidden items-center gap-2.5 py-2 text-xs text-[#525D63] lg:flex"
          >
            <span className="font-medium text-[#111618]">alex@support.example.org</span>
            <span className="h-3.5 w-px bg-[#DADDD8]" aria-hidden="true" />
            <span className="font-mono-code text-[#0F5C52]">support.example.org</span>
          </div>
        </div>
      </header>

      <main
        id="main-content"
        className={
          isLiveSessionView
            ? "flex flex-1 flex-col bg-[#0E1214]"
            : "mx-auto w-full max-w-6xl flex-1 px-4 py-6 sm:px-6"
        }
      >
        {activeTab === "devices" && (
          <DevicesScreen
            devices={devices}
            onConnect={startConnectionFlow}
            onOpenAddDevice={() => setIsAddDeviceOpen(true)}
            onToggleNotify={handleToggleNotify}
            onCreateSupportLink={(link) => {
              const dev =
                devices.find((d) => d.deviceId === link.deviceId) ?? selectedDevice;
              appendAuditEntry(
                dev,
                link.linkId,
                `SupportLinkIssued (15m TTL, ${link.serverDomain})`
              );
            }}
            onRedeemSupportLink={(linkId) => {
              appendAuditEntry(
                selectedDevice,
                linkId,
                `SupportLinkRedeemed (${selectedDevice.serverDomain})`
              );
            }}
            onBlockAndReport={(device, requester, category) => {
              appendAuditEntry(
                device,
                nextSessionId(),
                `AbuseReported & RequesterBlocked (${category}: ${requester})`
              );
            }}
          />
        )}

        {activeTab === "sessions" && (
          <SessionsScreen
            sessions={sessions}
            onOpenSession={(sess) => {
              const dev = devices.find((d) => d.deviceId === sess.deviceId);
              if (dev) setSelectedDeviceId(dev.deviceId);
              if (sess.status === "pending") {
                setActiveRequestSessionId(sess.sessionId);
                setRequestOutcome({ kind: "awaiting_approval", isJoinRequest: false });
                setActiveTab("requesting");
              } else {
                setEndedSummarySession(null);
                setActiveSessionId(sess.sessionId);
                setActiveTab("session");
              }
            }}
            onEndSession={(sessId) => {
              defaultTransport.send({
                type: "end_session",
                sessionId: sessId,
                targetDeviceId: selectedDevice.deviceId,
              });
              finalizeSessionEnd(sessId, "You ended the session");
            }}
            onViewSummary={(sess) => {
              setEndedSummarySession(sess);
              setActiveTab("session");
            }}
            onOpenAudit={() => setActiveTab("audit")}
          />
        )}

        {activeTab === "requesting" && (
          <>
            {requestOutcome.kind === "screen_unavailable" && (
              <ScreenUnavailableCard
                device={selectedDevice}
                reason={requestOutcome.reason}
                onChangeReason={(nextReason: ScreenUnavailableReason) =>
                  setRequestOutcome({ kind: "screen_unavailable", reason: nextReason })
                }
                onRetry={handleScreenRetry}
                onBack={() => setActiveTab("devices")}
              />
            )}
            {requestOutcome.kind === "owner_denied" && (
              <OwnerDeniedCard
                device={selectedDevice}
                streakCount={requestOutcome.streakCount}
                onRetry={() => startConnectionFlow(selectedDevice)}
                onBack={() => setActiveTab("devices")}
              />
            )}
            {requestOutcome.kind === "request_expired" && (
              <RequestExpiredCard
                device={selectedDevice}
                onRetry={() => startConnectionFlow(selectedDevice)}
                onBack={() => setActiveTab("devices")}
              />
            )}
            {requestOutcome.kind === "code_mismatch" && (
              <CodeMismatchCard
                device={selectedDevice}
                streakCount={requestOutcome.streakCount}
                onBack={() => setActiveTab("devices")}
              />
            )}
            {requestOutcome.kind === "device_key_changed" && (
              <DeviceKeyChangedAlert
                device={selectedDevice}
                expectedPubkey={requestOutcome.expectedPubkey}
                presentedPubkey={requestOutcome.presentedPubkey}
                onAcceptNewKey={handleAcceptRepinnedKey}
                onBack={() => setActiveTab("devices")}
              />
            )}
            {requestOutcome.kind === "control_plane_offline" && (
              <ControlPlaneOfflineCard
                reason={requestOutcome.reason}
                onRetry={() => {
                  setControlPlaneOffline(false);
                  startConnectionFlow(selectedDevice);
                }}
                onBack={() => setActiveTab("devices")}
              />
            )}
            {requestOutcome.kind === "awaiting_approval" && (
              <RequestingScreen
                device={selectedDevice}
                sasCode={sasCode}
                policy={policy}
                requestedPermissions={requestedPermissions}
                isJoinRequest={requestOutcome.isJoinRequest}
                onUpdateRequestedPermissions={setRequestedPermissions}
                onCancel={handleCancelRequest}
                onCodeMismatch={handleCodeMismatchAbort}
                onExpire={handleRequestExpired}
              />
            )}
          </>
        )}

        {activeTab === "session" && (
          <>
            {endedSummarySession ? (
              <SessionEndedSummary
                session={endedSummarySession}
                onViewAudit={() => setActiveTab("audit")}
                onBackToDevices={() => {
                  setEndedSummarySession(null);
                  setActiveTab("devices");
                }}
              />
            ) : (
              <ActiveSessionScreen
                device={
                  devices.find((d) => d.deviceId === activeSession.deviceId) ??
                  selectedDevice
                }
                session={activeSession}
                controlPlaneOffline={controlPlaneOffline}
                onRecheckHost={() => {
                  setSessions((prev) =>
                    prev.map((s) =>
                      s.sessionId === activeSession.sessionId
                        ? { ...s, hostOfflineMidSession: false }
                        : s
                    )
                  );
                }}
                onClipboardSync={(direction, _byteLen) => {
                  const dev =
                    devices.find((d) => d.deviceId === activeSession.deviceId) ??
                    selectedDevice;
                  appendAuditEntry(
                    dev,
                    activeSession.sessionId,
                    `ClipboardSyncEvent (${direction})`
                  );
                }}
                onFileTransferComplete={(filename, sizeBytes) => {
                  const dev =
                    devices.find((d) => d.deviceId === activeSession.deviceId) ??
                    selectedDevice;
                  appendAuditEntry(
                    dev,
                    activeSession.sessionId,
                    `FileTransferMetadata (${filename}, ${sizeBytes} B)`
                  );
                }}
                onEndSession={(reason = "You ended the session") => {
                  defaultTransport.send({
                    type: "end_session",
                    sessionId: activeSession.sessionId,
                    targetDeviceId: activeSession.deviceId,
                  });
                  finalizeSessionEnd(activeSession.sessionId, reason);
                }}
              />
            )}
          </>
        )}

        {activeTab === "policies" && (
          <PoliciesScreen policy={policy} onUpdatePolicy={setPolicy} />
        )}
        {activeTab === "audit" && <AuditLogScreen entries={auditEntries} />}
      </main>

      <AddDeviceWizard
        isOpen={isAddDeviceOpen}
        onClose={() => setIsAddDeviceOpen(false)}
        onAddDevice={handleAddDevice}
      />
    </div>
  );
};

export default App;
