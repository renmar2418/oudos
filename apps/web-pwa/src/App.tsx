import React, { useState } from "react";
import { FacingBracketsIcon } from "./components/FacingBracketsIcon";
import { INITIAL_POLICY, SAMPLE_AUDIT_ENTRIES, SAMPLE_DEVICES } from "./data/sampleData";
import { ActiveSessionScreen } from "./screens/ActiveSessionScreen";
import { AuditLogScreen } from "./screens/AuditLogScreen";
import { DevicesScreen } from "./screens/DevicesScreen";
import { PoliciesScreen } from "./screens/PoliciesScreen";
import { RequestingScreen } from "./screens/RequestingScreen";
import type { CoreTab, DeviceItem, PolicyState } from "./types";

const NAV_ITEMS: { id: CoreTab; label: string }[] = [
  { id: "devices", label: "Devices" },
  { id: "requesting", label: "Requesting" },
  { id: "session", label: "Active Session" },
  { id: "policies", label: "Policies" },
  { id: "audit", label: "Audit Log" },
];

export const App: React.FC<{ initialTab?: CoreTab }> = ({ initialTab = "devices" }) => {
  const [activeTab, setActiveTab] = useState<CoreTab>(initialTab);
  const [selectedDevice, setSelectedDevice] = useState<DeviceItem>(SAMPLE_DEVICES[0]!);
  const [policy, setPolicy] = useState<PolicyState>(INITIAL_POLICY);

  const handleConnect = (device: DeviceItem) => {
    setSelectedDevice(device);
    setActiveTab("requesting");
  };

  return (
    <div className="min-h-screen bg-[#F5F6F4] text-[#14181B]">
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-50 focus:rounded-lg focus:bg-[#0F7B6C] focus:px-4 focus:py-2 focus:text-sm focus:font-semibold focus:text-white"
      >
        Skip to main content
      </a>

      <header className="border-b border-[#DADDD8] bg-white">
        <div className="mx-auto flex max-w-6xl flex-wrap items-center justify-between gap-4 px-4 py-3 sm:px-6">
          <div className="flex items-center gap-2.5">
            <span className="text-[#0F7B6C]">
              <FacingBracketsIcon
                state={activeTab === "session" ? "in_session" : "idle"}
                className="h-7 w-7"
                label="Oudos"
              />
            </span>
            <span className="font-display text-xl font-semibold tracking-[-0.02em] text-[#14181B]">
              Oudos
            </span>
            <span className="rounded-md bg-[#E3F1EE] px-2 py-0.5 text-xs font-medium text-[#074A40]">
              v0.1 Asks-First
            </span>
          </div>

          <nav aria-label="Primary navigation" className="flex flex-wrap gap-1">
            {NAV_ITEMS.map((item) => {
              const current = activeTab === item.id;
              return (
                <button
                  key={item.id}
                  type="button"
                  aria-current={current ? "page" : undefined}
                  onClick={() => setActiveTab(item.id)}
                  className={`rounded-lg px-3 py-1.5 text-sm font-medium transition-colors focus:outline-none focus:ring-2 focus:ring-[#0F7B6C] ${
                    current
                      ? "bg-[#E3F1EE] text-[#074A40]"
                      : "text-[#4A524E] hover:bg-[#F5F6F4] hover:text-[#14181B]"
                  }`}
                >
                  {item.label}
                </button>
              );
            })}
          </nav>
        </div>
      </header>

      <main id="main-content" className="mx-auto max-w-6xl px-4 py-8 sm:px-6">
        {activeTab === "devices" && (
          <DevicesScreen devices={SAMPLE_DEVICES} onConnect={handleConnect} />
        )}
        {activeTab === "requesting" && (
          <RequestingScreen
            device={selectedDevice}
            sasCode="739 204"
            onCancel={() => setActiveTab("devices")}
            onSimulateAllow={() => setActiveTab("session")}
          />
        )}
        {activeTab === "session" && (
          <ActiveSessionScreen
            device={selectedDevice}
            onEndSession={() => setActiveTab("devices")}
          />
        )}
        {activeTab === "policies" && (
          <PoliciesScreen policy={policy} onUpdatePolicy={setPolicy} />
        )}
        {activeTab === "audit" && <AuditLogScreen entries={SAMPLE_AUDIT_ENTRIES} />}
      </main>
    </div>
  );
};

export default App;
