import { useMemo, useState } from "react";
import type { MediaItem } from "./creativeNavigation";
import {
  MAX_SESSION_VIDEO_SHOTS, MAX_VIDEO_SHOT_PROMPT, VIDEO_GENERATION_EXECUTION_QUALIFIED,
  cameraMotionLabel, moveVideoShot, normalizeVideoShot, videoIntentLabel,
  videoModeLabel, videoShotDuration,
  type VideoShot, type VideoShotInput, type VideoShotMode,
} from "./videoShotPlan";

type Props = {
  visible: boolean;
  media: readonly MediaItem[];
  onOpenLibrary: () => void;
  onOpenVideoStudio: () => void;
  onBrowseModels: (mode: VideoShotMode) => void;
};

const initialShot = (): VideoShotInput => ({
  mode: "text-to-video", prompt: "", referenceId: null,
  aspect: "16:9", durationSeconds: 4, intent: "general", camera: "static",
});

const modes: VideoShotMode[] = ["text-to-video", "image-to-video", "video-to-video"];

export default function VideoShotComposer({
  media, onOpenLibrary, onOpenVideoStudio, onBrowseModels, visible,
}: Props) {
  const [shots, setShots] = useState<VideoShot[]>([]);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState<VideoShotInput>(initialShot);
  const [notice, setNotice] = useState("");
  const photos = useMemo(() => media.filter((item) => item.kind === "image"), [media]);
  const videos = useMemo(() => media.filter((item) => item.kind === "video"), [media]);
  const references = draft.mode === "video-to-video" ? videos : photos;
  const totalSeconds = videoShotDuration(shots);

  const setField = <K extends keyof VideoShotInput>(key: K, value: VideoShotInput[K]) =>
    setDraft((previous) => ({ ...previous, [key]: value }));

  function addOrUpdateShot() {
    if (!editingId && shots.length >= MAX_SESSION_VIDEO_SHOTS) {
      setNotice("The session storyboard is limited to 24 planned shots."); return;
    }
    try {
      const normalized = normalizeVideoShot(draft, photos.map((x) => x.id), videos.map((x) => x.id));
      if (editingId) {
        if (!shots.some((shot) => shot.id === editingId)) {
          setNotice("That shot is no longer in the session plan."); return;
        }
        setShots((previous) => previous.map((shot) => shot.id === editingId ? { ...normalized, id: editingId } : shot));
      } else {
        setShots((previous) => previous.length >= MAX_SESSION_VIDEO_SHOTS
          ? previous : [...previous, { ...normalized, id: crypto.randomUUID() }]);
      }
      setEditingId(null);
      setDraft(initialShot());
      setNotice(editingId ? "Shot plan updated." : "Shot added to the session storyboard. Nothing was generated.");
    } catch (error) {
      setNotice(error instanceof Error ? error.message : "Could not plan this shot.");
    }
  }

  function selectShot(shot: VideoShot) {
    const { id, ...input } = shot;
    setEditingId(id);
    setDraft(input);
    setNotice("Editing this planned shot. No model job is running.");
  }

  function referenceText(referenceId: string) {
    const name = media.find((item) => item.id === referenceId)?.name;
    return name ? `Reference: ${name}` : "Reference no longer open. Edit this shot to choose another.";
  }

  function deleteShot(id: string) {
    setShots((previous) => previous.filter((shot) => shot.id !== id));
    if (editingId === id) { setEditingId(null); setDraft(initialShot()); }
    setNotice("Planned shot removed. No media was deleted.");
  }

  return <main className="video-compose-page" hidden={!visible}>
    <div className="page-eyebrow">CREATE VIDEOS · SHOT COMPOSER</div>
    <h1>Design the shot, then make the film.</h1>
    <p className="lead">Write what happens, set camera movement, and choose reference photos or clips.
      You can plan shots now. No video-generation model is qualified for execution in Optic Rift yet.</p>
    <div className="video-compose-status" role="status">
      <strong>Planning available · Video generation not yet runnable</strong>
      <p>Found models and cached configuration files are not installed or verified video engines.
        No video weights, remote inference, GPU jobs, or output files are started from this screen.</p>
    </div>
    <div className="video-compose-actions">
      <button type="button" className="secondary" onClick={onOpenLibrary}>Open Media Library</button>
      <button type="button" className="secondary" onClick={onOpenVideoStudio}>Open Video Studio</button>
      <button type="button" className="secondary" onClick={() => onBrowseModels(draft.mode)}>
        Explore {videoModeLabel[draft.mode]} models
      </button>
    </div>
    <div className="video-compose-layout">
      <section className="video-compose-form" aria-label="Shot settings">
        <h2>{editingId ? "Edit planned shot" : "Plan a shot"}</h2>
        <label>How will this shot be created?
          <select value={draft.mode} onChange={(event) => {
            setDraft((previous) => ({ ...previous, mode: event.target.value as VideoShotMode, referenceId: null }));
            setNotice("");
          }}>
            {modes.map((mode) => <option key={mode} value={mode}>{videoModeLabel[mode]}</option>)}
          </select>
        </label>
        <label>Describe the scene, action and visual style
          <textarea rows={5} maxLength={MAX_VIDEO_SHOT_PROMPT} value={draft.prompt}
            onChange={(event) => setField("prompt", event.target.value)}
            placeholder="A sailboat crosses a misty bay at dawn. Slow camera pan, soft natural light." />
          <span className="video-form-hint">{draft.prompt.length}/{MAX_VIDEO_SHOT_PROMPT} characters</span>
        </label>
        {draft.mode !== "text-to-video" && <label>
          {draft.mode === "image-to-video" ? "Reference photo" : "Source clip"}
          <select value={draft.referenceId ?? ""} onChange={(event) => setField("referenceId", event.target.value || null)}>
            <option value="">Choose a {draft.mode === "image-to-video" ? "photo" : "video"} from this session</option>
            {references.map((item) => <option value={item.id} key={item.id}>{item.name}</option>)}
          </select>
          {references.length === 0 && <span className="video-form-hint">No matching media opened. Import it in Media Library first.</span>}
        </label>}
        <div className="video-compose-fields">
          <label>Length
            <select value={draft.durationSeconds}
              onChange={(event) => setField("durationSeconds", Number(event.target.value))}>
              {[2, 4, 6, 8, 10, 12].map((seconds) =>
                <option key={seconds} value={seconds}>{seconds} seconds</option>)}
            </select>
          </label>
          <label>Frame shape
            <select value={draft.aspect} onChange={(event) => setField("aspect", event.target.value as VideoShotInput["aspect"])}>
              {["16:9", "9:16", "1:1", "4:3"].map((ratio) => <option value={ratio} key={ratio}>{ratio}</option>)}
            </select>
          </label>
          <label>Camera movement
            <select value={draft.camera} onChange={(event) => setField("camera", event.target.value as VideoShotInput["camera"])}>
              {Object.entries(cameraMotionLabel).map(([value, label]) =>
                <option value={value} key={value}>{label}</option>)}
            </select>
          </label>
          <label>Content intent
            <select value={draft.intent} onChange={(event) => setField("intent", event.target.value as VideoShotInput["intent"])}>
              {Object.entries(videoIntentLabel).map(([value, label]) =>
                <option value={value} key={value}>{label}</option>)}
            </select>
          </label>
        </div>
        <p className="video-form-hint">Content intent is a planning label, not an age check, official film rating,
          license permission or bypass of model safeguards. Adult-model discovery has a separate opt-in in Models.</p>
        {notice && <p role="status" className="video-compose-notice">{notice}</p>}
        <div className="video-compose-actions">
          <button type="button" className="primary" onClick={addOrUpdateShot}>
            {editingId ? "Update shot plan" : "Add to storyboard"}
          </button>
          {editingId && <button type="button" className="secondary"
            onClick={() => { setEditingId(null); setDraft(initialShot()); setNotice(""); }}>Cancel edit</button>}
        </div>
      </section>
      <section className="video-shot-board" aria-label="Session storyboard">
        <div className="video-shot-board-head">
          <div><h2>Shot list</h2><p>{shots.length}/{MAX_SESSION_VIDEO_SHOTS} planned shots · {totalSeconds} seconds</p></div>
          <span className="video-planning-tag">SESSION ONLY</span>
        </div>
        {shots.length === 0 && <div className="video-shot-empty">
          <strong>Start with the first shot.</strong>
          <p>Your shot list appears here. It is a plan, not a render queue. Plans are not saved after restart.</p>
        </div>}
        <ol className="video-shot-list">
          {shots.map((shot, index) => <li key={shot.id} className="video-shot-card">
            <div className="video-shot-number">SHOT {index + 1}</div>
            <strong>{videoModeLabel[shot.mode]} · {shot.durationSeconds}s · {shot.aspect}</strong>
            <p>{shot.prompt}</p>
            <small>{cameraMotionLabel[shot.camera]} · {videoIntentLabel[shot.intent]}</small>
            {shot.referenceId && <small>{referenceText(shot.referenceId)}</small>}
            <div className="video-shot-buttons">
              <button type="button" onClick={() => selectShot(shot)}>Edit</button>
              <button type="button" disabled={index === 0}
                onClick={() => setShots((previous) => moveVideoShot(previous, shot.id, -1))}>Move up</button>
              <button type="button" disabled={index === shots.length - 1}
                onClick={() => setShots((previous) => moveVideoShot(previous, shot.id, 1))}>Move down</button>
              <button type="button" onClick={() => deleteShot(shot.id)}>Remove</button>
            </div>
          </li>)}
        </ol>
        {!VIDEO_GENERATION_EXECUTION_QUALIFIED && <div className="video-compose-unsupported">
          <strong>Rendering not connected</strong>
          <p>Generating these shots requires a verified video model, licensed runtime, resource limits,
            and native job/export support. Exploring a model does not start execution.</p>
        </div>}
      </section>
    </div>
  </main>;
}
