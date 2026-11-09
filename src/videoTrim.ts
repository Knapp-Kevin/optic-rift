/**
 * Video Studio → Save trimmed video (issue #36). The WebView never sends a file path. The user
 * confirms the source in the native Windows dialog, and native code resolves the in/out
 * markers to real frames, encodes, validates and publishes a new MP4 to Pictures/Optic Rift.
 */
import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";

export type AudioPolicy = "none" | "keep-aac";

export type GrantInfo = {
  grantId: string;
  assetId: string;
  fileName: string;
  bytes: number;
  sha256: string;
  width: number;
  height: number;
  rotationDegrees: number;
  codec: string;
  frameCount: number;
  durationSeconds: number;
  audio: AudioPolicy;
  tool: { source: string; ffmpegVersion: string; ffmpegSha256: string; ffprobeSha256: string };
};

export type FrameRef = { index: number; pts: number; seconds: number };

export type TrimPlan = {
  grantId: string;
  sourceName: string;
  first: FrameRef;
  last: FrameRef;
  frameCount: number;
  durationSeconds: number;
  width: number;
  height: number;
  audio: AudioPolicy;
};

export type JobState =
  | "encoding" | "validating" | "publishing" | "cancelling"
  | "succeeded" | "failed" | "cancelled" | "interrupted";

export type JobStatus = {
  jobId: string;
  state: JobState;
  sourceName: string;
  framesDone: number;
  frameCount: number;
  message: string;
  output: null | {
    fileName: string; sha256: string; bytes: number; width: number; height: number;
    frameCount: number; durationSeconds: number; hasAudio: boolean;
  };
  startedUnixMs: number;
  finishedUnixMs: number | null;
};

export const FINAL_STATES: readonly JobState[] = ["succeeded", "failed", "cancelled", "interrupted"];
export const isFinal = (state: JobState) => FINAL_STATES.includes(state);

export const audioText = (audio: AudioPolicy) =>
  audio === "keep-aac" ? "Audio: the clip's audio track is kept (re-encoded to AAC)." : "Audio: none (the clip has no audio track).";

export function jobProgress(job: JobStatus): number {
  if (job.state === "succeeded") return 1;
  if (!job.frameCount) return 0;
  return Math.min(1, job.framesDone / job.frameCount);
}

export function jobLabel(job: JobStatus): string {
  switch (job.state) {
    case "encoding": return `Encoding ${job.framesDone} of ${job.frameCount} frames`;
    case "validating": return "Checking the exported frames";
    case "publishing": return "Saving to Pictures";
    case "cancelling": return "Stopping the export";
    case "succeeded": return "Trimmed video saved";
    case "failed": return "Video export failed";
    case "cancelled": return "Video export cancelled";
    case "interrupted": return "Video export interrupted";
  }
}

const errorText = (reason: unknown) =>
  typeof reason === "string" ? reason : reason instanceof Error ? reason.message : "The video export did not complete.";

/** App-lifetime state for the single video export job, so leaving Video Studio never hides it. */
export function useVideoExport() {
  const [job, setJob] = useState<JobStatus | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const [interruptedNotice, setInterruptedNotice] = useState("");
  const polling = useRef<number | null>(null);

  // After a restart, tell the user about an export that was interrupted (its files are gone).
  useEffect(() => {
    void invoke<JobStatus[]>("video_trim_jobs").then((jobs) => {
      const recent = jobs.filter((j) => j.state === "interrupted")
        .sort((a, b) => (b.finishedUnixMs ?? 0) - (a.finishedUnixMs ?? 0))[0];
      if (recent && Date.now() - (recent.finishedUnixMs ?? 0) < 10 * 60 * 1000) {
        setInterruptedNotice(`An export of ${recent.sourceName} was interrupted when Optic Rift closed. Its partial files were removed; no video was saved.`);
      }
    }).catch(() => undefined);
  }, []);

  const running = job !== null && !isFinal(job.state);
  useEffect(() => {
    if (!running || !job) return;
    const id = job.jobId;
    polling.current = window.setInterval(() => {
      void invoke<JobStatus | null>("video_trim_status", { jobId: id }).then((status) => {
        if (status) setJob(status);
      }).catch(() => undefined);
    }, 400);
    return () => { if (polling.current !== null) window.clearInterval(polling.current); };
  }, [running, job?.jobId]);

  const start = useCallback(async (request: {
    grantId: string; assetId: string; inSeconds: number; outSeconds: number | null;
    expectedFirstIndex: number; expectedLastIndex: number;
  }) => {
    const status = await invoke<JobStatus>("video_trim_start", {
      request: { ...request, acceptPicturesExport: true },
    });
    setDismissed(false);
    setJob(status);
    return status;
  }, []);

  const cancel = useCallback(async () => {
    if (!job || isFinal(job.state)) return;
    try { setJob(await invoke<JobStatus>("video_trim_cancel", { jobId: job.jobId })); }
    catch (reason) { setJob({ ...job, message: errorText(reason) }); }
  }, [job]);

  const reveal = useCallback(async () => {
    if (job?.state === "succeeded") await invoke("video_export_reveal", { jobId: job.jobId });
  }, [job]);

  return {
    job, running, dismissed, interruptedNotice, start, cancel, reveal,
    dismiss: () => { setDismissed(true); setInterruptedNotice(""); },
  };
}

export type VideoExport = ReturnType<typeof useVideoExport>;

export async function grantSource(asset: {
  id: string; name: string; size: number; lastModifiedMs?: number;
}): Promise<GrantInfo | null> {
  if (asset.lastModifiedMs === undefined) {
    throw new Error("This clip was not opened from a file, so it can't be exported.");
  }
  return invoke<GrantInfo | null>("video_source_grant", {
    request: { assetId: asset.id, name: asset.name, size: asset.size, lastModifiedMs: asset.lastModifiedMs },
  });
}

export const resolveTrim = (request: { grantId: string; assetId: string; inSeconds: number; outSeconds: number | null }) =>
  invoke<TrimPlan>("video_trim_resolve", { request });

export const revokeSource = (grantId: string) =>
  invoke<boolean>("video_source_revoke", { grantId }).catch(() => false);

export { errorText as videoExportError };
