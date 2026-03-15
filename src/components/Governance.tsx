import { useEffect, useState } from "react";
import { useStore } from "../store";
import * as tauri from "../lib/tauri";
import type { GovernanceAllowlistEntry, GovernanceRequest } from "../lib/types";

type Tab = "overview" | "allowlist" | "requests" | "audit";

export default function Governance() {
  const {
    governanceStatus,
    governanceAllowlist,
    governanceRequests,
    governanceAuditLog,
    governanceLoading,
    refreshGovernanceStatus,
    refreshGovernanceAllowlist,
    refreshGovernanceRequests,
    refreshGovernanceAuditLog,
    showToast,
  } = useStore();

  const [tab, setTab] = useState<Tab>("overview");
  const [pinInput, setPinInput] = useState("");
  const [isAdmin, setIsAdmin] = useState(false);
  const [showPinDialog, setShowPinDialog] = useState(false);
  const [pendingAction, setPendingAction] = useState<(() => Promise<void>) | null>(null);

  // Setup form
  const [setupPin, setSetupPin] = useState("");
  const [setupPinConfirm, setSetupPinConfirm] = useState("");

  // Add to allowlist form
  const [addServerId, setAddServerId] = useState("");
  const [addDisplayName, setAddDisplayName] = useState("");
  const [addDescription, setAddDescription] = useState("");
  const [addNotes, setAddNotes] = useState("");

  // Request form
  const [requestServerId, setRequestServerId] = useState("");
  const [requestServerName, setRequestServerName] = useState("");
  const [requestReason, setRequestReason] = useState("");

  // Review
  const [reviewNotes, setReviewNotes] = useState("");

  useEffect(() => {
    refreshGovernanceStatus();
    refreshGovernanceAllowlist();
    refreshGovernanceRequests();
    refreshGovernanceAuditLog();
  }, []);

  const requirePin = (action: () => Promise<void>) => {
    if (isAdmin) {
      action();
    } else {
      setPendingAction(() => action);
      setShowPinDialog(true);
    }
  };

  const submitPin = async () => {
    const valid = await tauri.verifyAdminPin(pinInput);
    if (valid) {
      setIsAdmin(true);
      setShowPinDialog(false);
      if (pendingAction) {
        await pendingAction();
        setPendingAction(null);
      }
    } else {
      showToast("Invalid admin PIN", "error");
    }
    setPinInput("");
  };

  const handleSetup = async () => {
    if (setupPin !== setupPinConfirm) {
      showToast("PINs do not match", "error");
      return;
    }
    if (setupPin.length < 4) {
      showToast("PIN must be at least 4 characters", "error");
      return;
    }
    try {
      await tauri.setupGovernance(setupPin);
      setIsAdmin(true);
      setSetupPin("");
      setSetupPinConfirm("");
      showToast("Governance mode activated", "success");
      refreshGovernanceStatus();
      refreshGovernanceAuditLog();
    } catch (e) {
      showToast(`Setup failed: ${e}`, "error");
    }
  };

  const handleToggleGovernance = async (enabled: boolean) => {
    requirePin(async () => {
      try {
        await tauri.setGovernanceEnabled(enabled, pinInput || "");
        showToast(enabled ? "Governance enabled" : "Governance disabled", "success");
        refreshGovernanceStatus();
        refreshGovernanceAuditLog();
      } catch (e) {
        showToast(`Failed: ${e}`, "error");
      }
    });
  };

  const handleAddToAllowlist = async () => {
    if (!addServerId.trim() || !addDisplayName.trim()) {
      showToast("Server identifier and display name are required", "error");
      return;
    }
    requirePin(async () => {
      try {
        await tauri.governanceAddToAllowlist({
          adminPin: pinInput || "",
          serverIdentifier: addServerId.trim(),
          displayName: addDisplayName.trim(),
          description: addDescription.trim() || undefined,
          reviewNotes: addNotes.trim() || undefined,
        });
        setAddServerId("");
        setAddDisplayName("");
        setAddDescription("");
        setAddNotes("");
        showToast("Server added to allowlist", "success");
        refreshGovernanceAllowlist();
        refreshGovernanceStatus();
        refreshGovernanceAuditLog();
      } catch (e) {
        showToast(`Failed: ${e}`, "error");
      }
    });
  };

  const handleRemoveFromAllowlist = async (entry: GovernanceAllowlistEntry) => {
    requirePin(async () => {
      try {
        await tauri.governanceRemoveFromAllowlist(pinInput || "", entry.server_identifier);
        showToast("Removed from allowlist", "success");
        refreshGovernanceAllowlist();
        refreshGovernanceStatus();
        refreshGovernanceAuditLog();
      } catch (e) {
        showToast(`Failed: ${e}`, "error");
      }
    });
  };

  const handleCreateRequest = async () => {
    if (!requestServerId.trim() || !requestServerName.trim()) {
      showToast("Server identifier and name are required", "error");
      return;
    }
    try {
      await tauri.governanceCreateRequest({
        serverIdentifier: requestServerId.trim(),
        serverName: requestServerName.trim(),
        requestReason: requestReason.trim() || undefined,
      });
      setRequestServerId("");
      setRequestServerName("");
      setRequestReason("");
      showToast("Approval request submitted", "success");
      refreshGovernanceRequests();
      refreshGovernanceStatus();
      refreshGovernanceAuditLog();
    } catch (e) {
      showToast(`Failed: ${e}`, "error");
    }
  };

  const handleReviewRequest = async (request: GovernanceRequest, approved: boolean) => {
    requirePin(async () => {
      try {
        await tauri.governanceReviewRequest({
          adminPin: pinInput || "",
          requestId: request.id,
          approved,
          reviewNotes: reviewNotes.trim() || undefined,
        });
        setReviewNotes("");
        showToast(approved ? "Request approved" : "Request denied", "success");
        refreshGovernanceRequests();
        refreshGovernanceAllowlist();
        refreshGovernanceStatus();
        refreshGovernanceAuditLog();
      } catch (e) {
        showToast(`Failed: ${e}`, "error");
      }
    });
  };

  const tabs: { id: Tab; label: string }[] = [
    { id: "overview", label: "Overview" },
    { id: "allowlist", label: "Allowlist" },
    { id: "requests", label: "Requests" },
    { id: "audit", label: "Audit Log" },
  ];

  // If governance has never been set up, show setup screen
  if (governanceStatus && !governanceStatus.has_admin_pin) {
    return (
      <div className="max-w-lg mx-auto mt-12">
        <h1 className="text-2xl font-bold text-brightwing-gray-100 mb-2">Registry Governance</h1>
        <p className="text-brightwing-gray-400 mb-6">
          Set up governance to control which MCP servers can be installed.
          Only servers on the approved allowlist will be installable.
        </p>
        <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-6 space-y-4">
          <h2 className="text-lg font-semibold text-brightwing-gray-200">Create Admin PIN</h2>
          <p className="text-sm text-brightwing-gray-400">
            This PIN will be required to manage the allowlist, review requests, and change governance settings.
          </p>
          <div>
            <label className="block text-sm text-brightwing-gray-300 mb-1">Admin PIN</label>
            <input
              type="password"
              value={setupPin}
              onChange={(e) => setSetupPin(e.target.value)}
              placeholder="Enter PIN (min 4 characters)"
              className="w-full bg-brightwing-gray-900 border border-brightwing-gray-600 rounded px-3 py-2 text-brightwing-gray-100 text-sm"
            />
          </div>
          <div>
            <label className="block text-sm text-brightwing-gray-300 mb-1">Confirm PIN</label>
            <input
              type="password"
              value={setupPinConfirm}
              onChange={(e) => setSetupPinConfirm(e.target.value)}
              placeholder="Confirm PIN"
              className="w-full bg-brightwing-gray-900 border border-brightwing-gray-600 rounded px-3 py-2 text-brightwing-gray-100 text-sm"
            />
          </div>
          <button
            onClick={handleSetup}
            disabled={setupPin.length < 4 || setupPin !== setupPinConfirm}
            className="w-full bg-brightwing-blue hover:bg-brightwing-blue/80 disabled:opacity-40 text-white text-sm font-medium py-2 px-4 rounded transition-colors"
          >
            Activate Governance
          </button>
        </div>
      </div>
    );
  }

  return (
    <div>
      {/* PIN Dialog */}
      {showPinDialog && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-6 w-80">
            <h3 className="text-lg font-semibold text-brightwing-gray-100 mb-3">Admin PIN Required</h3>
            <input
              type="password"
              value={pinInput}
              onChange={(e) => setPinInput(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && submitPin()}
              placeholder="Enter admin PIN"
              autoFocus
              className="w-full bg-brightwing-gray-900 border border-brightwing-gray-600 rounded px-3 py-2 text-brightwing-gray-100 text-sm mb-4"
            />
            <div className="flex gap-2">
              <button
                onClick={() => { setShowPinDialog(false); setPinInput(""); setPendingAction(null); }}
                className="flex-1 bg-brightwing-gray-700 hover:bg-brightwing-gray-600 text-brightwing-gray-300 text-sm py-2 rounded transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={submitPin}
                className="flex-1 bg-brightwing-blue hover:bg-brightwing-blue/80 text-white text-sm py-2 rounded transition-colors"
              >
                Submit
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-brightwing-gray-100">Registry Governance</h1>
          <p className="text-sm text-brightwing-gray-400 mt-1">
            {governanceStatus?.policy_org
              ? `${governanceStatus.policy_org} — Managed server allowlist`
              : "Control which MCP servers are approved for installation"}
          </p>
          {governanceStatus?.policy_enforced && (
            <p className="text-xs text-amber-400 mt-1">
              Enforced by external policy — cannot be disabled from this app
            </p>
          )}
        </div>
        {governanceStatus && (
          <div className="flex items-center gap-3">
            <span className={`inline-flex items-center gap-1.5 text-sm font-medium px-3 py-1 rounded-full ${
              governanceStatus.enabled
                ? "bg-green-500/20 text-green-400"
                : "bg-yellow-500/20 text-yellow-400"
            }`}>
              <span className={`w-2 h-2 rounded-full ${governanceStatus.enabled ? "bg-green-400" : "bg-yellow-400"}`} />
              {governanceStatus.enabled ? "Enforcing" : "Disabled"}
            </span>
            {isAdmin && !governanceStatus.policy_enforced && (
              <button
                onClick={() => handleToggleGovernance(!governanceStatus.enabled)}
                className="text-xs text-brightwing-gray-400 hover:text-brightwing-gray-200 underline"
              >
                {governanceStatus.enabled ? "Disable" : "Enable"}
              </button>
            )}
          </div>
        )}
      </div>

      {/* Tabs */}
      <div className="flex gap-1 mb-6 border-b border-brightwing-gray-700">
        {tabs.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={`px-4 py-2 text-sm font-medium transition-colors border-b-2 -mb-px ${
              tab === t.id
                ? "border-brightwing-blue text-brightwing-blue"
                : "border-transparent text-brightwing-gray-400 hover:text-brightwing-gray-200"
            }`}
          >
            {t.label}
            {t.id === "requests" && governanceStatus && governanceStatus.pending_requests > 0 && (
              <span className="ml-1.5 bg-red-500 text-white text-xs px-1.5 py-0.5 rounded-full">
                {governanceStatus.pending_requests}
              </span>
            )}
          </button>
        ))}
      </div>

      {/* Overview Tab */}
      {tab === "overview" && governanceStatus && (
        <div className="grid grid-cols-3 gap-4 mb-6">
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-5">
            <div className="text-3xl font-bold text-brightwing-gray-100">{governanceStatus.allowlist_count}</div>
            <div className="text-sm text-brightwing-gray-400 mt-1">Approved Servers</div>
          </div>
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-5">
            <div className="text-3xl font-bold text-brightwing-gray-100">{governanceStatus.pending_requests}</div>
            <div className="text-sm text-brightwing-gray-400 mt-1">Pending Requests</div>
          </div>
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-5">
            <div className={`text-3xl font-bold ${governanceStatus.enabled ? "text-green-400" : "text-yellow-400"}`}>
              {governanceStatus.enabled ? "Active" : "Off"}
            </div>
            <div className="text-sm text-brightwing-gray-400 mt-1">Enforcement</div>
          </div>

          <div className="col-span-3 bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-5">
            <h3 className="text-sm font-semibold text-brightwing-gray-300 mb-3">How Governance Works</h3>
            <ul className="space-y-2 text-sm text-brightwing-gray-400">
              <li className="flex gap-2">
                <span className="text-brightwing-blue font-bold">1.</span>
                Admin enables governance and adds approved MCP servers to the allowlist
              </li>
              <li className="flex gap-2">
                <span className="text-brightwing-blue font-bold">2.</span>
                Users can only install servers that appear on the allowlist
              </li>
              <li className="flex gap-2">
                <span className="text-brightwing-blue font-bold">3.</span>
                Users can submit requests to add new servers — admins review and approve/deny
              </li>
              <li className="flex gap-2">
                <span className="text-brightwing-blue font-bold">4.</span>
                All actions are recorded in the audit log for compliance
              </li>
            </ul>
          </div>

          {!isAdmin && (
            <div className="col-span-3">
              <button
                onClick={() => setShowPinDialog(true)}
                className="bg-brightwing-gray-700 hover:bg-brightwing-gray-600 text-brightwing-gray-200 text-sm py-2 px-4 rounded transition-colors"
              >
                Sign in as Admin
              </button>
            </div>
          )}
        </div>
      )}

      {/* Allowlist Tab */}
      {tab === "allowlist" && (
        <div>
          {isAdmin && (
            <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-5 mb-6">
              <h3 className="text-sm font-semibold text-brightwing-gray-200 mb-3">Add Server to Allowlist</h3>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-xs text-brightwing-gray-400 mb-1">Server Identifier *</label>
                  <input
                    value={addServerId}
                    onChange={(e) => setAddServerId(e.target.value)}
                    placeholder="e.g. github-mcp-server"
                    className="w-full bg-brightwing-gray-900 border border-brightwing-gray-600 rounded px-3 py-1.5 text-brightwing-gray-100 text-sm"
                  />
                </div>
                <div>
                  <label className="block text-xs text-brightwing-gray-400 mb-1">Display Name *</label>
                  <input
                    value={addDisplayName}
                    onChange={(e) => setAddDisplayName(e.target.value)}
                    placeholder="e.g. GitHub MCP Server"
                    className="w-full bg-brightwing-gray-900 border border-brightwing-gray-600 rounded px-3 py-1.5 text-brightwing-gray-100 text-sm"
                  />
                </div>
                <div>
                  <label className="block text-xs text-brightwing-gray-400 mb-1">Description</label>
                  <input
                    value={addDescription}
                    onChange={(e) => setAddDescription(e.target.value)}
                    placeholder="Brief description"
                    className="w-full bg-brightwing-gray-900 border border-brightwing-gray-600 rounded px-3 py-1.5 text-brightwing-gray-100 text-sm"
                  />
                </div>
                <div>
                  <label className="block text-xs text-brightwing-gray-400 mb-1">Review Notes</label>
                  <input
                    value={addNotes}
                    onChange={(e) => setAddNotes(e.target.value)}
                    placeholder="Why this server was approved"
                    className="w-full bg-brightwing-gray-900 border border-brightwing-gray-600 rounded px-3 py-1.5 text-brightwing-gray-100 text-sm"
                  />
                </div>
              </div>
              <button
                onClick={handleAddToAllowlist}
                disabled={!addServerId.trim() || !addDisplayName.trim()}
                className="mt-3 bg-green-600 hover:bg-green-500 disabled:opacity-40 text-white text-sm font-medium py-1.5 px-4 rounded transition-colors"
              >
                Add to Allowlist
              </button>
            </div>
          )}

          {governanceLoading ? (
            <p className="text-sm text-brightwing-gray-400">Loading...</p>
          ) : governanceAllowlist.length === 0 ? (
            <p className="text-sm text-brightwing-gray-400">No servers on the allowlist yet.</p>
          ) : (
            <div className="space-y-2">
              {governanceAllowlist.map((entry) => (
                <div
                  key={entry.id}
                  className="flex items-center justify-between bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg px-4 py-3"
                >
                  <div>
                    <div className="text-sm font-medium text-brightwing-gray-100">{entry.display_name}</div>
                    <div className="text-xs text-brightwing-gray-400">
                      {entry.server_identifier}
                      {entry.description && ` — ${entry.description}`}
                    </div>
                    <div className="text-xs text-brightwing-gray-500 mt-0.5">
                      Approved by {entry.approved_by} on {new Date(entry.approved_at).toLocaleDateString()}
                      {entry.review_notes && ` — "${entry.review_notes}"`}
                    </div>
                  </div>
                  {isAdmin && (
                    <button
                      onClick={() => handleRemoveFromAllowlist(entry)}
                      className="text-red-400 hover:text-red-300 text-xs transition-colors"
                    >
                      Remove
                    </button>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Requests Tab */}
      {tab === "requests" && (
        <div>
          {/* User request form */}
          <div className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg p-5 mb-6">
            <h3 className="text-sm font-semibold text-brightwing-gray-200 mb-3">Request a Server</h3>
            <p className="text-xs text-brightwing-gray-400 mb-3">
              Submit a request to add an MCP server to the approved list.
            </p>
            <div className="grid grid-cols-3 gap-3">
              <div>
                <label className="block text-xs text-brightwing-gray-400 mb-1">Server Identifier *</label>
                <input
                  value={requestServerId}
                  onChange={(e) => setRequestServerId(e.target.value)}
                  placeholder="e.g. slack-mcp"
                  className="w-full bg-brightwing-gray-900 border border-brightwing-gray-600 rounded px-3 py-1.5 text-brightwing-gray-100 text-sm"
                />
              </div>
              <div>
                <label className="block text-xs text-brightwing-gray-400 mb-1">Server Name *</label>
                <input
                  value={requestServerName}
                  onChange={(e) => setRequestServerName(e.target.value)}
                  placeholder="e.g. Slack MCP Server"
                  className="w-full bg-brightwing-gray-900 border border-brightwing-gray-600 rounded px-3 py-1.5 text-brightwing-gray-100 text-sm"
                />
              </div>
              <div>
                <label className="block text-xs text-brightwing-gray-400 mb-1">Reason</label>
                <input
                  value={requestReason}
                  onChange={(e) => setRequestReason(e.target.value)}
                  placeholder="Why do you need this server?"
                  className="w-full bg-brightwing-gray-900 border border-brightwing-gray-600 rounded px-3 py-1.5 text-brightwing-gray-100 text-sm"
                />
              </div>
            </div>
            <button
              onClick={handleCreateRequest}
              disabled={!requestServerId.trim() || !requestServerName.trim()}
              className="mt-3 bg-brightwing-blue hover:bg-brightwing-blue/80 disabled:opacity-40 text-white text-sm font-medium py-1.5 px-4 rounded transition-colors"
            >
              Submit Request
            </button>
          </div>

          {/* Pending requests */}
          {governanceRequests.length === 0 ? (
            <p className="text-sm text-brightwing-gray-400">No approval requests.</p>
          ) : (
            <div className="space-y-2">
              {governanceRequests.map((req) => (
                <div
                  key={req.id}
                  className="bg-brightwing-gray-800 border border-brightwing-gray-700 rounded-lg px-4 py-3"
                >
                  <div className="flex items-center justify-between">
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-brightwing-gray-100">{req.server_name}</span>
                        <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${
                          req.status === "pending"
                            ? "bg-yellow-500/20 text-yellow-400"
                            : req.status === "approved"
                            ? "bg-green-500/20 text-green-400"
                            : "bg-red-500/20 text-red-400"
                        }`}>
                          {req.status}
                        </span>
                      </div>
                      <div className="text-xs text-brightwing-gray-400 mt-0.5">
                        {req.server_identifier} — requested by {req.requested_by} on{" "}
                        {new Date(req.requested_at).toLocaleDateString()}
                      </div>
                      {req.request_reason && (
                        <div className="text-xs text-brightwing-gray-500 mt-0.5">
                          Reason: {req.request_reason}
                        </div>
                      )}
                      {req.review_notes && (
                        <div className="text-xs text-brightwing-gray-500 mt-0.5">
                          Admin notes: {req.review_notes}
                        </div>
                      )}
                    </div>
                    {isAdmin && req.status === "pending" && (
                      <div className="flex items-center gap-2">
                        <input
                          value={reviewNotes}
                          onChange={(e) => setReviewNotes(e.target.value)}
                          placeholder="Notes..."
                          className="bg-brightwing-gray-900 border border-brightwing-gray-600 rounded px-2 py-1 text-xs text-brightwing-gray-100 w-32"
                        />
                        <button
                          onClick={() => handleReviewRequest(req, true)}
                          className="bg-green-600 hover:bg-green-500 text-white text-xs py-1 px-3 rounded transition-colors"
                        >
                          Approve
                        </button>
                        <button
                          onClick={() => handleReviewRequest(req, false)}
                          className="bg-red-600 hover:bg-red-500 text-white text-xs py-1 px-3 rounded transition-colors"
                        >
                          Deny
                        </button>
                      </div>
                    )}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Audit Log Tab */}
      {tab === "audit" && (
        <div>
          {governanceAuditLog.length === 0 ? (
            <p className="text-sm text-brightwing-gray-400">No audit log entries.</p>
          ) : (
            <div className="space-y-1">
              {governanceAuditLog.map((entry) => (
                <div
                  key={entry.id}
                  className="flex items-center gap-3 px-4 py-2 bg-brightwing-gray-800 border border-brightwing-gray-700 rounded text-xs"
                >
                  <span className="text-brightwing-gray-500 w-36 shrink-0">
                    {new Date(entry.timestamp).toLocaleString()}
                  </span>
                  <span className={`w-28 shrink-0 font-medium ${
                    entry.action.includes("blocked") || entry.action.includes("denied")
                      ? "text-red-400"
                      : entry.action.includes("approved") || entry.action.includes("enabled") || entry.action.includes("add")
                      ? "text-green-400"
                      : "text-brightwing-gray-300"
                  }`}>
                    {entry.action}
                  </span>
                  <span className="text-brightwing-gray-400">{entry.actor}</span>
                  {entry.target_server && (
                    <span className="text-brightwing-gray-300 font-mono">{entry.target_server}</span>
                  )}
                  {entry.detail && (
                    <span className="text-brightwing-gray-500 truncate">{entry.detail}</span>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
