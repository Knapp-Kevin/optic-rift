import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { formatBytes, licenseAcknowledgement, type InstallEvent, type Inspection } from "./models";

/**
 * App-lifetime ownership of the single native model download (issue #27).
 *
 * Owned above the route-scoped Models page, so leaving Models, changing the adult-discovery
 * preference or inspecting another model never drops progress or Cancel. Native
 * `model_install_status` is the source of truth for reconciliation after a remount or reload.
 * State is in memory only: no download history is written anywhere.
 */
export type DownloadSession = {
  installId: string; repo: string; revision: string;
  /** Source "not-for-all-audiences" flag; hides the model name while adult discovery is off. */
  flagged: boolean;
  progress: InstallEvent | null;
  cancelling: boolean;
  /** Adopted from native status (e.g. after a reload): no local promise will end it. */
  adopted: boolean;
};
export type DownloadError = { repo: string; revision: string; flagged: boolean; message: string };
type NativeStatus = { install_id: string; repo: string; revision: string; source_flagged: boolean; cancel_requested: boolean; last: InstallEvent | null };

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

  // Native truth: adopt an unknown active download, drop an adopted one that has ended.
  const reconcile = useCallback(async () => {
    let status: NativeStatus | null;
    try { status = await invoke<NativeStatus | null>("model_install_status"); } catch { return; }
    update((s) => {
      if (status && (!s || s.installId !== status.install_id)) {
        if (s && !s.adopted) return s; // our own promise still owns it; it will settle
        return { installId: status.install_id, repo: status.repo, revision: status.revision, flagged: status.source_flagged, progress: status.last, cancelling: status.cancel_requested, adopted: true };
      }
      if (status && s) return { ...s, flagged: s.flagged || status.source_flagged, progress: status.last ?? s.progress, cancelling: s.cancelling || status.cancel_requested };
      if (!status && s?.adopted) return null;
      return s;
    });
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
    update({ installId, repo: detail.id, revision: detail.revision, flagged, progress: null, cancelling: false, adopted: false });
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
    if (!s || s.cancelling) return;
    update((prev) => (prev?.installId === s.installId ? { ...prev, cancelling: true } : prev));
    // Always the exact live id from the session ref, never one captured by an old render.
    void invoke<boolean>("model_install_cancel", { installId: s.installId })
      .then((accepted) => { if (!accepted) void reconcile(); })
      .catch(() => void reconcile());
  }, [update, reconcile]);

  const value = useMemo(() => ({ session, error, start, cancel, dismissError: () => setError(null) }), [session, error, start, cancel]);
  return <DownloadContext.Provider value={value}>{children}</DownloadContext.Provider>;
}

export function useDownloads(): Downloads {
  const value = useContext(DownloadContext);
  if (!value) throw new Error("useDownloads needs DownloadProvider");
  return value;
}

/** Model name, or a neutral label for a source-flagged model while adult discovery is off. */
export function downloadLabel(item: { repo: string; flagged: boolean }, includeAdult: boolean) {
  return item.flagged && !includeAdult ? "a model" : item.repo;
}

export function DownloadProgress({ session }: { session: DownloadSession }) {
  const { cancel } = useDownloads();
  const p = session.progress;
  return <>
    {p && <><progress max={p.bytes_total || 1} value={p.bytes_done} aria-label="Download progress" />
      <span>{p.state} {p.file ? `· ${p.file} (${p.file_index + 1}/${p.file_count})` : ""} · {formatBytes(p.bytes_done)} of {formatBytes(p.bytes_total)}</span></>}
    {!p && <span>{session.cancelling ? "Cancelling…" : "Starting download…"}</span>}
    <button type="button" className="secondary" disabled={session.cancelling} onClick={cancel}>{session.cancelling ? "Cancelling…" : "Cancel download"}</button>
  </>;
}

/** Compact status card for the active download, or its last error, used on every page. */
export function DownloadStatus({ includeAdult, compact = false }: { includeAdult: boolean; compact?: boolean }) {
  const { session, error, dismissError } = useDownloads();
  if (session) {
    return <div className={compact ? "model-progress download-status compact" : "model-progress download-status"} role="status" aria-label="Model download in progress">
      <strong>Downloading {downloadLabel(session, includeAdult)}</strong>
      <DownloadProgress session={session} />
    </div>;
  }
  if (error) {
    return <div className={compact ? "model-progress download-status compact" : "model-progress download-status"} role="alert">
      <strong>Download of {downloadLabel(error, includeAdult)} stopped</strong>
      <span>{error.message}</span>
      <button type="button" className="secondary" onClick={dismissError}>Dismiss</button>
    </div>;
  }
  return null;
}
