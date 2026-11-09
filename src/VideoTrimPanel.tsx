import { useEffect, useState } from "react";
import type { MediaItem } from "./creativeNavigation";
import {
  audioText, grantSource, isFinal, jobLabel, jobProgress, resolveTrim, videoExportError,
  type GrantInfo, type VideoExport,
} from "./videoTrim";

type Props = {
  clip: MediaItem | null;
  ready: boolean;
  inSeconds: number;
  outSeconds: number | null;
  grant: GrantInfo | undefined;
  onGrant: (grant: GrantInfo) => void;
  /** The native side no longer honours this clip's grant; forget it so Save asks again. */
  onGrantInvalid: () => void;
  exporter: VideoExport;
  formatTime: (seconds: number) => string;
};

/** Save trimmed video: an explicit, consent-gated native export of the in/out range. */
export default function VideoTrimPanel({ clip, ready, inSeconds, outSeconds, grant, onGrant, onGrantInvalid, exporter, formatTime }: Props) {
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState("");
  const { job, running } = exporter;
  const jobForClip = job && !exporter.dismissed ? job : null;
  // A notice belongs to the clip it was about; switching clips clears it.
  useEffect(() => { setNotice(""); }, [clip?.id]);

  async function saveTrimmed() {
    if (!clip || busy || running) return;
    setBusy(true);
    setNotice("");
    try {
      let source = grant;
      if (!source) {
        setNotice("Confirm the source file in the Windows dialog…");
        const granted = await grantSource(clip);
        if (!granted) { setNotice("No file was chosen. Nothing was exported."); return; }
        onGrant(granted);
        source = granted;
      }
      const request = { grantId: source.grantId, assetId: clip.id, inSeconds, outSeconds };
      const plan = await resolveTrim(request);
      const approved = window.confirm(
        `Save a trimmed copy of ${plan.sourceName}?\n\n` +
        `Frames ${plan.first.index + 1} to ${plan.last.index + 1} (${plan.frameCount} frames, ` +
        `${formatTime(plan.first.seconds)} to ${formatTime(plan.last.seconds)}, ${plan.width}×${plan.height}).\n` +
        `${audioText(plan.audio)}\n\n` +
        "A new MP4 and a receipt will be saved in Pictures\\Optic Rift, which may sync with OneDrive. " +
        "Your original video is not changed.",
      );
      if (!approved) { setNotice("Export declined. Nothing was written."); return; }
      setNotice("");
      await exporter.start({
        ...request, expectedFirstIndex: plan.first.index, expectedLastIndex: plan.last.index,
      });
    } catch (reason) {
      const text = videoExportError(reason);
      // A changed source or a lost grant must be confirmed again in the dialog next time.
      if (/changed on disk|not authorized/.test(text)) onGrantInvalid();
      setNotice(text);
    } finally {
      setBusy(false);
    }
  }

  const disabled = !clip || !ready || busy || running;
  return <div className="video-trim-panel">
    <strong>Save trimmed video</strong>
    <p>Saves the frames from the in marker to the out marker as a new MP4. Optic Rift re-encodes
      the exact frames you marked; the original file is never changed.</p>
    {clip && <p className="video-trim-range">Range: {formatTime(inSeconds)} to {outSeconds === null ? "end" : formatTime(outSeconds)}
      {grant ? ` · source confirmed (${grant.frameCount} frames)` : ""}</p>}
    <button type="button" className="primary full" onClick={() => void saveTrimmed()} disabled={disabled}
      title={running ? "Another video export is running" : !ready ? "Wait for the video to load" : "Save a trimmed copy"}>
      {busy ? "Preparing…" : "Save trimmed video…"}
    </button>
    {notice && <p role="status" className="video-trim-notice">{notice}</p>}
    {exporter.interruptedNotice && !jobForClip && <div role="status" className="video-trim-notice">
      {exporter.interruptedNotice}{" "}
      <button type="button" className="inline-link" onClick={exporter.dismiss}>Dismiss</button>
    </div>}
    {jobForClip && <div className={`video-trim-job state-${jobForClip.state}`} role="status" aria-live="polite">
      <div className="video-trim-job-head"><strong>{jobLabel(jobForClip)}</strong><span>{jobForClip.sourceName}</span></div>
      {!isFinal(jobForClip.state) && <progress max={1} value={jobProgress(jobForClip)} aria-label="Video export progress" />}
      <p>{jobForClip.message}</p>
      <div className="video-trim-job-actions">
        {!isFinal(jobForClip.state) && <button type="button" className="secondary" disabled={jobForClip.state === "cancelling"}
          onClick={() => void exporter.cancel()}>Cancel export</button>}
        {jobForClip.state === "succeeded" && <button type="button" className="secondary"
          onClick={() => void exporter.reveal().catch((e) => setNotice(videoExportError(e)))}>Show in folder</button>}
        {isFinal(jobForClip.state) && <button type="button" className="secondary" onClick={exporter.dismiss}>Dismiss</button>}
      </div>
    </div>}
    <p className="video-form-hint">Video export requires verified FFmpeg (bundled in release builds). The exported copy is not yet an editable project
      asset. Audio is kept only when the clip has exactly one track; there are no audio editing tools.</p>
  </div>;
}

/** Compact, app-wide indicator so a running export is never hidden by navigation. */
export function VideoExportChip({ exporter, onOpen }: { exporter: VideoExport; onOpen: () => void }) {
  const { job } = exporter;
  if (!job || exporter.dismissed) return null;
  const pct = Math.round(jobProgress(job) * 100);
  return <div className={`video-export-chip state-${job.state}`} role="status" aria-live="polite">
    <button type="button" className="video-export-chip-label" onClick={onOpen} title="Open Video Studio">
      Video export · {isFinal(job.state) ? jobLabel(job) : `${pct}%`}
    </button>
    {!isFinal(job.state)
      ? <button type="button" className="video-export-chip-cancel" disabled={job.state === "cancelling"} onClick={() => void exporter.cancel()}>Cancel</button>
      : <button type="button" className="video-export-chip-cancel" aria-label="Dismiss video export status" onClick={exporter.dismiss}>×</button>}
  </div>;
}
