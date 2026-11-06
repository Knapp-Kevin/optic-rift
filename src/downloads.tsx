import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { formatBytes, licenseAcknowledgement, type InstallEvent, type Inspection } from "./models";
import { concealed, downloadLabel, mergeStatus, type CancelState, type DownloadError, type DownloadSession, type NativeStatus } from "./downloadState";

export { concealed, downloadLabel, type CancelState, type DownloadError, type DownloadSession };

/**
 * App-lifetime ownership of the single native model download (issue #27).
 *
 * Owned above the route-scoped Models page, so leaving Models, changing the adult-discovery
 * preference or inspecting another model never drops progress or Cancel. Native
 * `model_install_status` is the source of truth for reconciliation after a remount or reload.
 * State is in memory only: no download history is written anywhere.
 */

type Downloads = {
  session: DownloadSession | null;
  error: DownloadError | null;
  /** Resolves true when the model was installed. Refused while another download runs. */
  start: (inspection: Inspection, accept: boolean) => Promise<boolean>;
  cancel: () => void;
  dismissError: () => void;
};

const DownloadContext = createContext<Downloads | null>(null);
const errorText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : "The desktop bridge is unavailable.");
const TERMINAL = new Set(["installed", "cancelled", "failed"]);

export function DownloadProvider({ children, onInstalled }: { children: ReactNode; onInstalled: () => void }) {
  const [session, setSession] = useState<DownloadSession | null>(null);
  const [error, setError] = useState<DownloadError | null>(null);
  const current = useRef<DownloadSession | null>(null);
  const cancelEpoch = useRef(0);
  const onInstalledRef = useRef(onInstalled);
  onInstalledRef.current = onInstalled;

  const update = useCallback((next: DownloadSession | null | ((s: DownloadSession | null) => DownloadSession | null)) => {
    // A direct value is visible to synchronous callers at once, so a double click cannot
    // start two installs before React re-renders.
    if (typeof next !== "function") current.current = next;
    setSession((prev) => {
      const value = typeof next === "function" ? next(prev) : next;
      current.current = value;
      return value;
    });
  }, []);

  const reconcile = useCallback(async () => {
    const epoch = cancelEpoch.current;
    let status: NativeStatus | null;
    try { status = await invoke<NativeStatus | null>("model_install_status"); } catch { return; }
    update((s) => mergeStatus(s, status, epoch !== cancelEpoch.current));
  }, [update]);

  // Exactly one app-lifetime subscription to native progress.
  useEffect(() => {
    const stop = listen<InstallEvent>("model-install", (e) => {
      update((s) => {
        if (!s || e.payload.install_id !== s.installId) return s; // stale or foreign id
        if (s.adopted && TERMINAL.has(e.payload.state)) return null;
        return { ...s, progress: e.payload };
      });
    });
    void reconcile();
    return () => { void stop.then((f) => f()); };
  }, [update, reconcile]);

  // While a download is shown, keep it honest against native state (ended, reloaded, IPC lost).
  const sessionId = session?.installId ?? null;
  useEffect(() => {
    if (!sessionId) return;
    const timer = window.setInterval(() => void reconcile(), 2000);
    return () => window.clearInterval(timer);
  }, [sessionId, reconcile]);

  const start = useCallback(async (inspection: Inspection, accept: boolean) => {
    const { detail } = inspection;
    const plan = detail.plans[0];
    if (!plan || current.current) return false;
    const installId = crypto.randomUUID();
    const flagged = detail.audience === "not-for-all-audiences";
    setError(null);
    update({ installId, repo: detail.id, revision: detail.revision, flagged, progress: null, cancel: "idle", adopted: false });
    try {
      await invoke("model_install", { request: { install_id: installId, repo: detail.id, revision: detail.revision, adapter: plan.plan.adapter, license_acknowledged: licenseAcknowledgement(detail), accept } });
      onInstalledRef.current();
      return true;
    } catch (e) {
      setError({ repo: detail.id, revision: detail.revision, flagged, message: errorText(e) });
      return false;
    } finally {
      update((s) => (s?.installId === installId ? null : s));
    }
  }, [update]);

  const cancel = useCallback(() => {
    const s = current.current;
    if (!s || s.cancel !== "idle") return;
    const id = s.installId;
    const epoch = ++cancelEpoch.current;
    const settle = (next: CancelState) => {
      if (cancelEpoch.current === epoch) cancelEpoch.current++; // invalidate polls issued meanwhile
      update((prev) => (prev?.installId === id && prev.cancel === "pending" ? { ...prev, cancel: next } : prev));
    };
    update((prev) => (prev?.installId === id ? { ...prev, cancel: "pending" } : prev));
    // Always the exact live id from the session ref, never one captured by an old render.
    void invoke<boolean>("model_install_cancel", { installId: id })
      .then((accepted) => { settle(accepted ? "accepted" : "idle"); if (!accepted) void reconcile(); })
      .catch(() => { settle("idle"); void reconcile(); });
  }, [update, reconcile]);

  const value = useMemo(() => ({ session, error, start, cancel, dismissError: () => setError(null) }), [session, error, start, cancel]);
  return <DownloadContext.Provider value={value}>{children}</DownloadContext.Provider>;
}

export function useDownloads(): Downloads {
  const value = useContext(DownloadContext);
  if (!value) throw new Error("useDownloads needs DownloadProvider");
  return value;
}

export function DownloadProgress({ session, includeAdult }: { session: DownloadSession; includeAdult: boolean }) {
  const { cancel } = useDownloads();
  const p = session.progress;
  // While identity is concealed, show only state and byte counts, never a source file path or message.
  const hide = concealed(session, includeAdult);
  const busy = session.cancel !== "idle";
  return <>
    {p && <><progress max={p.bytes_total || 1} value={p.bytes_done} aria-label="Download progress" />
      <span>{p.state}{!hide && p.file ? ` · ${p.file} (${p.file_index + 1}/${p.file_count})` : hide ? ` · file ${p.file_index + 1} of ${p.file_count}` : ""} · {formatBytes(p.bytes_done)} of {formatBytes(p.bytes_total)}</span></>}
    {!p && <span>{busy ? "Cancelling…" : "Starting download…"}</span>}
    <button type="button" className="secondary" disabled={busy} onClick={cancel}>{busy ? "Cancelling…" : "Cancel download"}</button>
  </>;
}

/** Compact status card for the active download, or its last error, used on every page. */
export function DownloadStatus({ includeAdult, compact = false }: { includeAdult: boolean; compact?: boolean }) {
  const { session, error, dismissError } = useDownloads();
  const cls = compact ? "model-progress download-status compact" : "model-progress download-status";
  if (session) {
    return <div className={cls} role="status" aria-label="Model download in progress">
      <strong>Downloading {downloadLabel(session, includeAdult)}</strong>
      <DownloadProgress session={session} includeAdult={includeAdult} />
    </div>;
  }
  if (error) {
    // Native messages can name repository files; hide them with the identity.
    return <div className={cls} role="alert">
      <strong>Download of {downloadLabel(error, includeAdult)} stopped</strong>
      <span>{concealed(error, includeAdult) ? "Turn on adult discovery to see details." : error.message}</span>
      <button type="button" className="secondary" onClick={dismissError}>Dismiss</button>
    </div>;
  }
  return null;
}
