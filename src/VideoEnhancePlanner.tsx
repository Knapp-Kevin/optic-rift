import { useState } from "react";
import type { MediaItem } from "./creativeNavigation";
import {
  MAX_VIDEO_ENHANCE_NOTES, VIDEO_ENHANCEMENT_EXECUTION_QUALIFIED,
  normalizeVideoEnhancementPlan, videoEnhanceOperations,
  type EnhanceOperation, type VideoEnhancementPlan,
} from "./videoEnhancementPlan";

type Props = {
  visible: boolean;
  media: readonly MediaItem[];
  onOpenLibrary: () => void;
  onOpenVideoStudio: () => void;
  onBrowseModels: () => void;
};

export default function VideoEnhancePlanner({
  media, onOpenLibrary, onOpenVideoStudio, onBrowseModels, visible,
}: Props) {
  const videos = media.filter((item) => item.kind === "video");
  const [draft, setDraft] = useState<VideoEnhancementPlan>({
    videoId: null, operations: [], target: "original", priority: "preserve-detail", notes: "",
  });
  const [reviewed, setReviewed] = useState<VideoEnhancementPlan | null>(null);
  const [notice, setNotice] = useState("");

  function toggle(operation: EnhanceOperation) {
    setReviewed(null);
    setDraft((previous) => ({
      ...previous,
      operations: previous.operations.includes(operation)
        ? previous.operations.filter((item) => item !== operation)
        : [...previous.operations, operation],
    }));
  }

  function reviewPlan() {
    try {
      const plan = normalizeVideoEnhancementPlan(draft, videos.map((video) => video.id));
      setReviewed(plan);
      setNotice("Enhancement plan ready for review. No processing or output was started.");
    } catch (error) {
      setReviewed(null);
      setNotice(error instanceof Error ? error.message : "Could not prepare the plan.");
    }
  }

  return <main className="video-compose-page video-enhance-page" hidden={!visible}>
    <div className="page-eyebrow">VIDEO QUALITY & RESTORATION</div>
    <h1>Improve the footage, preserve the original.</h1>
    <p className="lead">Choose the quality problems you want to address. The plan helps specify
      the intended result, but no enhancement renderer has been qualified yet.</p>
    <div className="video-compose-status" role="status">
      <strong>Quality-improvement planning available · Processing not available</strong>
      <p>Upscaling, denoise, restoration, stabilization and motion interpolation still require verified
        native or model-based execution. Selecting them here does not process your video or contact a provider.</p>
    </div>
    <div className="video-compose-actions">
      <button type="button" className="secondary" onClick={onOpenLibrary}>Open Media Library</button>
      <button type="button" className="secondary" onClick={onOpenVideoStudio}>Inspect in Video Studio</button>
      <button type="button" className="secondary" onClick={onBrowseModels}>Explore video-to-video models</button>
    </div>
    <div className="video-compose-layout">
      <section className="video-compose-form" aria-label="Enhancement plan">
        <h2>Source and goals</h2>
        <label>Choose video
          <select value={draft.videoId ?? ""} onChange={(event) => {
            setDraft((previous) => ({ ...previous, videoId: event.target.value || null }));
            setReviewed(null);
          }}>
            <option value="">Select an opened local video</option>
            {videos.map((video) => <option key={video.id} value={video.id}>{video.name}</option>)}
          </select>
          {videos.length === 0 && <span className="video-form-hint">Import footage into Media Library first.</span>}
        </label>
        <fieldset className="video-enhance-choices">
          <legend>What needs improvement?</legend>
          {videoEnhanceOperations.map((operation) => <label key={operation.id} className="video-enhance-option">
            <input type="checkbox" checked={draft.operations.includes(operation.id)}
              onChange={() => toggle(operation.id)} />
            <span><strong>{operation.name}</strong><small>{operation.description}</small></span>
          </label>)}
        </fieldset>
        <div className="video-compose-fields">
          <label>Target resolution
            <select value={draft.target} onChange={(event) => {
              setDraft((previous) => ({ ...previous, target: event.target.value as VideoEnhancementPlan["target"] }));
              setReviewed(null);
            }}>
              <option value="original">Keep source dimensions</option>
              <option value="1080p">Plan for 1080p</option>
              <option value="2160p">Plan for 4K</option>
            </select>
          </label>
          <label>Quality priority
            <select value={draft.priority} onChange={(event) => {
              setDraft((previous) => ({ ...previous, priority: event.target.value as VideoEnhancementPlan["priority"] }));
              setReviewed(null);
            }}>
              <option value="preserve-detail">Preserve source detail</option>
              <option value="balanced">Balanced</option>
              <option value="maximum-improvement">Maximum improvement</option>
            </select>
          </label>
        </div>
        <label>Notes about the desired result
          <textarea rows={3} value={draft.notes} maxLength={MAX_VIDEO_ENHANCE_NOTES}
            onChange={(event) => { setDraft((previous) => ({ ...previous, notes: event.target.value })); setReviewed(null); }}
            placeholder="Reduce low-light grain but preserve the texture of the water…" />
        </label>
        <p className="video-form-hint">This is not an export setting. A target like 4K does not imply the
          available model can generate genuine 4K detail, or that any adapter can run locally.</p>
        {notice && <p role="status" className="video-compose-notice">{notice}</p>}
        <button type="button" className="primary" onClick={reviewPlan}>Review enhancement plan</button>
      </section>
      <section className="video-shot-board" aria-label="Enhancement plan review">
        <h2>Planned treatment</h2>
        {!reviewed && <div className="video-shot-empty">
          <strong>No reviewed plan yet.</strong>
          <p>Choose a source and one or more improvements. The plan stays in this session,
            and no media files will be changed by selecting options.</p>
        </div>}
        {reviewed && <div className="video-enhance-review">
          <p><strong>Source:</strong> {videos.find((v) => v.id === reviewed.videoId)?.name ?? "Unavailable"}</p>
          <p><strong>Target:</strong> {reviewed.target} · <strong>Priority:</strong> {reviewed.priority.replaceAll("-", " ")}</p>
          <ol>{reviewed.operations.map((id) => {
            const step = videoEnhanceOperations.find((op) => op.id === id);
            return <li key={id}><strong>{step?.name ?? id}</strong>
              <span>{step?.risk}</span></li>;
          })}</ol>
          {reviewed.notes && <p><strong>Notes:</strong> {reviewed.notes}</p>}
          <p className="video-form-hint">No restoration, upscaling, or video export has occurred.
            This review is not a saved project recipe or proof that effects can run.</p>
        </div>}
        {!VIDEO_ENHANCEMENT_EXECUTION_QUALIFIED && <div className="video-compose-unsupported">
          <strong>Enhancement execution not connected</strong>
          <p>We still need a tested decoder, verified enhancement adapters, a safe native job queue,
            before/after quality comparisons, export, and source-preserving provenance.
            Audio creation and mixing remain outside Optic Rift's scope.</p>
        </div>}
      </section>
    </div>
  </main>;
}
