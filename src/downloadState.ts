import type { InstallEvent } from "./models";

/** Pure download-session state rules (issue #27), kept free of React and IPC so tests can run them. */

/**
 * - `idle`: Cancel available.
 * - `pending`: our cancel call is in flight.
 * - `accepted`: native confirmed it or reports `cancel_requested`.
 * A rejected or failed call returns to `idle` for that exact install, so Cancel is never stuck.
 */
export type CancelState = "idle" | "pending" | "accepted";

export type DownloadSession = {
  installId: string; repo: string; revision: string;
  /** Source "not-for-all-audiences" flag. `null` = not yet classified (treated as concealed). */
  flagged: boolean | null;
  progress: InstallEvent | null;
  cancel: CancelState;
  /** Adopted from native status (e.g. after a reload): no local promise will end it. */
  adopted: boolean;
};
export type DownloadError = { repo: string; revision: string; flagged: boolean | null; message: string };
export type NativeStatus = { install_id: string; repo: string; revision: string; source_flagged: boolean | null; cancel_requested: boolean; last: InstallEvent | null };

/**
 * Model identity is concealed while adult discovery is off unless the source is known to be
 * unflagged. An unclassified download (before the pinned inspection) counts as concealed.
 */
export function concealed(item: { flagged: boolean | null }, includeAdult: boolean) {
  return !includeAdult && item.flagged !== false;
}

/** Model name, or a neutral label when its identity is concealed. */
export function downloadLabel(item: { repo: string; flagged: boolean | null }, includeAdult: boolean) {
  return concealed(item, includeAdult) ? "a model" : item.repo;
}

/** Pure merge of native status into the UI session; native settles cancellation. Exported for tests. */
export function mergeStatus(s: DownloadSession | null, status: NativeStatus | null, cancelChangedSinceRequest: boolean): DownloadSession | null {
  if (status && (!s || s.installId !== status.install_id)) {
    if (s && !s.adopted) return s; // our own promise still owns it; it will settle
    return { installId: status.install_id, repo: status.repo, revision: status.revision, flagged: status.source_flagged, progress: status.last, cancel: status.cancel_requested ? "accepted" : "idle", adopted: true };
  }
  if (status && s) {
    // Native truth. A pending call keeps "pending" until it resolves. A poll issued before our
    // latest cancel action never downgrades it (the poll race).
    let cancel: CancelState;
    if (status.cancel_requested) cancel = "accepted";
    else if (s.cancel === "pending" || cancelChangedSinceRequest) cancel = s.cancel;
    else cancel = "idle";
    // A known classification only becomes stricter or resolves from unknown; never flagged -> unflagged.
    const flagged = s.flagged === true || status.source_flagged === true ? true : status.source_flagged ?? s.flagged;
    return { ...s, flagged, progress: status.last ?? s.progress, cancel };
  }
  if (!status && s?.adopted) return null;
  return s;
}

