import { useEffect, useRef, useState, type ChangeEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import ProjectChooser from "./ProjectChooser";
import HuggingFaceAccess from "./HuggingFaceAccess";
import ModelBrowser from "./ModelBrowser";
import GeneratePanel from "./GeneratePanel";
import GenerationHistory from "./GenerationHistory";
import type { ProjectRecord } from "./projects";
import { hasQualifiedLocalImageModel, type ModelLibrary } from "./models";
import { formatSize, IMAGE_PREVIEW_FAILED, VIDEO_PREVIEW_FAILED } from "./mediaFormat";
import { renderAdjustedPhoto } from "./photoEdits";

type View = "projects" | "studio" | "models" | "settings";
type MediaKind = "image" | "video";
type MediaItem = { id: string; name: string; kind: MediaKind; url: string; size: number };
type RuntimeStatus = { application: string; stage: string; generation_ready: boolean; project_storage_ready: boolean };

const formatTime = (seconds: number) => {
  const safe = Math.max(0, Number.isFinite(seconds) ? seconds : 0);
  const m = Math.floor(safe / 60);
  const s = Math.floor(safe % 60);
  return String(m).padStart(2, "0") + ":" + String(s).padStart(2, "0");
};

export default function App() {
  const [view, setView] = useState<View>("projects");
  const [project, setProject] = useState<ProjectRecord | null>(null);
  const [items, setItems] = useState<MediaItem[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [prompt, setPrompt] = useState("");
  const [studioMode, setStudioMode] = useState<"create" | "edit">("edit");
  const [studioModeChosen, setStudioModeChosen] = useState(false);
  const [createOpened, setCreateOpened] = useState(false);
  const [creationMode, setCreationMode] = useState<"text-to-image" | "image-to-image">("text-to-image");
  const [libraryVersion, setLibraryVersion] = useState(0);
  const [historyVersion, setHistoryVersion] = useState(0);
  const [message, setMessage] = useState("");
  const [includeAdultModels, setIncludeAdultModels] = useState(false);
  const [brightness, setBrightness] = useState(100);
  const [contrast, setContrast] = useState(100);
  const [rotation, setRotation] = useState(0);
  const [photoExportBusy, setPhotoExportBusy] = useState(false);
  const [photoExportNotice, setPhotoExportNotice] = useState("");
  const [duration, setDuration] = useState(0);
  const [playhead, setPlayhead] = useState(0);
  const [trimIn, setTrimIn] = useState(0);
  const [trimOut, setTrimOut] = useState<number | null>(null);
  const [status, setStatus] = useState<RuntimeStatus | null>(null);
  const [failedIds, setFailedIds] = useState<ReadonlySet<string>>(new Set());
  const inputRef = useRef<HTMLInputElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const objectUrls = useRef<string[]>([]);
  const selected = items.find((item) => item.id === selectedId) ?? null;
  const previewFailed = selected ? failedIds.has(selected.id) : false;
  const isVideo = selected?.kind === "video" && !previewFailed;
  function markFailed(id: string, text: string) {
    setFailedIds((previous) => new Set(previous).add(id));
    setMessage(text);
  }

  useEffect(() => {
    void invoke<RuntimeStatus>("runtime_status")
      .then(setStatus)
      .catch(() => setStatus(null));
    return () => {
      objectUrls.current.forEach((url) => URL.revokeObjectURL(url));
      objectUrls.current = [];
    };
  }, []);

  useEffect(() => {
    if (view !== "studio" || studioModeChosen) return;
    let active = true;
    void invoke<ModelLibrary>("models_library")
      .then((lib) => { if (active) setStudioMode(hasQualifiedLocalImageModel(lib) ? "create" : "edit"); })
      .catch(() => { if (active) setStudioMode("edit"); });
    return () => { active = false; };
  }, [view, libraryVersion, studioModeChosen]);

  useEffect(() => { if (view === "studio" && studioMode === "create") setCreateOpened(true); }, [view, studioMode]);

  function selectItem(id: string) {
    setSelectedId(id);
    setDuration(0);
    setPlayhead(0);
    setTrimIn(0);
    setTrimOut(null);
    setBrightness(100);
    setContrast(100);
    setRotation(0);
    setPhotoExportNotice("");
    setMessage("");
  }

  function addMedia(event: ChangeEvent<HTMLInputElement>) {
    const files = Array.from(event.currentTarget.files ?? []);
    event.currentTarget.value = "";
    const accepted: MediaItem[] = [];
    for (const file of files.slice(0, 50)) {
      const kind = file.type.startsWith("video/") ? "video" : file.type.startsWith("image/") ? "image" : null;
      if (!kind) continue;
      const url = URL.createObjectURL(file);
      objectUrls.current.push(url);
      accepted.push({ id: crypto.randomUUID(), name: file.name, kind, url, size: file.size });
    }
    if (accepted.length > 0) {
      setItems((previous) => [...previous, ...accepted]);
      if (!selectedId) selectItem(accepted[0].id);
      setMessage("Media is open for this session only. Durable project saving is not implemented yet.");
    } else {
      setMessage("No supported image or video files were selected.");
    }
  }

  function addGenerated(result: { name: string; url: string; size: number }) {
    objectUrls.current.push(result.url);
    const item: MediaItem = { id: crypto.randomUUID(), name: result.name, kind: "image", url: result.url, size: result.size };
    setItems((previous) => [...previous, item]);
    selectItem(item.id);
    setMessage("Generated image opened in this session. Its file and provenance stay in Optic Rift's output folder; use Save to Pictures to keep a copy elsewhere.");
  }

  async function exportAdjustedPhoto() {
    if (!selected || selected.kind !== "image" || previewFailed || photoExportBusy) return;
    // Pictures can be redirected to OneDrive. Deliberately ask before even
    // preparing the copy; no silent export or photo upload from this workflow.
    const allowed = window.confirm(
      "Export an adjusted PNG copy to Pictures / Optic Rift? Windows may sync Pictures with OneDrive. This does not save a project asset, and the original image stays unchanged."
    );
    if (!allowed) return;
    setPhotoExportBusy(true);
    setPhotoExportNotice("Rendering and validating your edited photo…");
    try {
      const png = await renderAdjustedPhoto(selected.url, { brightness, contrast, rotation });
      const receipt = await invoke<{ path: string; sha256: string; width: number; height: number }>("photo_export_copy", {
        request: {
          ...png, brightness, contrast, rotation,
          accept_pictures_export: true,
        },
      });
      setPhotoExportNotice(`Saved adjusted PNG copy: ${receipt.path}. Your original was not changed.`);
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      setPhotoExportNotice(`Export not completed: ${reason}`);
    } finally {
      setPhotoExportBusy(false);
    }
  }

  function seek(to: number) {
    const value = Math.min(Math.max(to, 0), duration || 0);
    if (videoRef.current) videoRef.current.currentTime = value;
    setPlayhead(value);
  }

  function setInPoint() {
    setTrimIn(Math.min(playhead, trimOut ?? duration));
  }
  function setOutPoint() {
    setTrimOut(Math.max(playhead, trimIn));
  }
  function removeItem(id: string) {
    const entry = items.find((item) => item.id === id);
    if (!entry) return;
    setItems((previous) => previous.filter((item) => item.id !== id));
    if (selectedId === id) selectItem(items.find((item) => item.id !== id)?.id ?? "");
    URL.revokeObjectURL(entry.url);
    objectUrls.current = objectUrls.current.filter((url) => url !== entry.url);
  }

  return (
    <div className="app">
      <aside className="navigation">
        <div className="brand"><img className="brand-icon-img" src="/branding/optic-rift-approved-256.webp" alt="" /><span>OPTIC <b>RIFT</b><small>VISUAL STUDIO</small></span></div>
        <div className="nav-group">
          <div className="group-label">WORKSPACE</div>
          <button className={view === "projects" ? "nav selected" : "nav"} onClick={() => setView("projects")}>▤ <span>Projects</span></button>
          <button className={view === "studio" ? "nav selected" : "nav"} onClick={() => setView("studio")}>▣ <span>Studio</span></button>
          <button className={view === "models" ? "nav selected" : "nav"} onClick={() => setView("models")}>◈ <span>Models</span></button>
          <button className={view === "settings" ? "nav selected" : "nav"} onClick={() => setView("settings")}>⚙ <span>Settings</span></button>
        </div>
        <div className="nav-footer">
          <span className="status-dot" /> Local-first
          <small>Pre-alpha architecture preview</small>
        </div>
      </aside>

      <div className="shell">
        <header className="topbar">
          <div className="breadcrumbs">OPTIC RIFT <span>/</span> {view === "projects" ? "Projects" : view === "studio" ? "Creative studio" : view === "models" ? "Model library" : "Preferences"}</div>
          <div className="top-actions"><button className="project-pill" onClick={() => setView("projects")} title="Choose a project">{project ? "Project: " + project.name : "No project open"}</button><span className="pill">PRE-ALPHA</span><button className="top-button" onClick={() => setView("models")}>Model manager</button></div>
        </header>

        {view === "projects" && <ProjectChooser current={project} onOpened={(opened) => { setProject(opened); setView("studio"); }} />}

        {view === "studio" && <>
          <nav className="studio-tabs" aria-label="Studio workspace">
            <div className="studio-tab-switch" role="group" aria-label="Choose Studio activity">
              <button type="button" className={studioMode === "create" ? "selected" : ""} aria-pressed={studioMode === "create"}
                onClick={() => { setStudioMode("create"); setStudioModeChosen(true); }}>Create</button>
              <button type="button" className={studioMode === "edit" ? "selected" : ""} aria-pressed={studioMode === "edit"}
                onClick={() => { setStudioMode("edit"); setStudioModeChosen(true); }}>Edit media</button>
            </div>
            <span>{studioMode === "create" ? "Generate local images. Video generation models are discoverable in Models." : "Edit the media you have opened."}</span>
          </nav>
        </>}
        {/* Create stays mounted once opened, so a running job, its progress/Cancel and the current result
            survive a switch to Edit media or another page. Hidden content is not rendered visibly. */}
        {createOpened && <main className="create-workspace" hidden={!(view === "studio" && studioMode === "create")}>
            <GeneratePanel layout="create" visible={view === "studio" && studioMode === "create"} prompt={prompt} onPromptChange={setPrompt} mode={creationMode}
              onModeChange={setCreationMode} historyVersion={historyVersion} projectId={project?.id ?? null}
              libraryVersion={libraryVersion} onOpenModels={() => setView("models")}
              onResult={(media) => { addGenerated(media); setStudioMode("edit"); setStudioModeChosen(true); }}
              onGenerated={() => setHistoryVersion((v) => v + 1)} />
          </main>}
        {view === "studio" && studioMode === "edit" && <main className="workspace">
          <section className="media-panel">
            <div className="section-title"><div><h2>Media library</h2><p>Your images and clips</p></div><span className="count">{items.length}</span></div>
            <input ref={inputRef} type="file" accept="image/*,video/*" multiple hidden onChange={addMedia} aria-label="Select image and video files" />
            <button className="import-button" onClick={() => inputRef.current?.click()}>＋ Import media</button>
            <div className="media-list">
              {items.length === 0 && <div className="empty-library">Your canvas starts here.<br /><span>Import a video or image to preview it.</span></div>}
              {items.map((item) => <div className={selectedId === item.id ? "media-row active" : "media-row"} key={item.id}>
                <button onClick={() => selectItem(item.id)} className="media-pick">
                  <span className="media-kind">{item.kind === "video" ? "▶" : "▧"}</span><span className="media-label"><strong title={item.name}>{item.name}</strong><small>{item.kind} · {formatSize(item.size)}{failedIds.has(item.id) ? " · can't preview" : ""}</small></span>
                </button>
                <button className="remove-item" aria-label={"Remove " + item.name} title="Remove from this session" onClick={() => removeItem(item.id)}>×</button>
              </div>)}
            </div>
            <div className="sidebar-note">Imported files stay on your computer. Imported media is open for this session only and is not saved into projects yet.</div>
          </section>

          <section className="studio-center">
            <div className="canvas-header"><div><strong>Preview</strong><span>{selected ? selected.name : "No media selected"}</span></div><span className="preview-label">{selected?.kind ?? "READY"}</span></div>
            <div className="canvas">
              {selected && previewFailed && <div className="empty-canvas preview-failed" role="alert"><div className="empty-icon">⚠</div><h1>Can't preview {selected.name}</h1><p>{selected.kind === "image" ? IMAGE_PREVIEW_FAILED : VIDEO_PREVIEW_FAILED}</p></div>}
              {selected?.kind === "image" && !previewFailed && <img key={selected.id} onError={() => markFailed(selected.id, IMAGE_PREVIEW_FAILED)} src={selected.url} alt={selected.name} style={{ filter: "brightness(" + brightness + "%) contrast(" + contrast + "%)", transform: "rotate(" + rotation + "deg)" }} />}
              {selected?.kind === "video" && !previewFailed && <video key={selected.id} ref={videoRef} src={selected.url} controls preload="metadata" playsInline onLoadedMetadata={(event) => { const length = event.currentTarget.duration; setDuration(Number.isFinite(length) ? length : 0); setPlayhead(0); }} onTimeUpdate={(event) => setPlayhead(event.currentTarget.currentTime)} onError={() => markFailed(selected.id, VIDEO_PREVIEW_FAILED)} />}
              {!selected && <div className="empty-canvas"><div className="empty-icon">◉</div><h1>Make something extraordinary.</h1><p>Start with a photo or video, or choose Create to generate an image locally.</p><button className="primary" onClick={() => inputRef.current?.click()}>Import your first clip</button></div>}
            </div>
            <div className="player-controls">
              {isVideo ? <><button className="control" onClick={() => seek(playhead - 0.1)} title="Step back 0.1 seconds">↶ 0.1s</button><button className="control" onClick={() => seek(playhead + 0.1)} title="Step forward 0.1 seconds">0.1s ↷</button><span className="time">{formatTime(playhead)} / {formatTime(duration)}</span></> :
                <span className="time">{previewFailed ? "Preview unavailable" : selected ? "Image preview" : "Import media to begin"}</span>}
              <span className="control-hint">Originals never modified</span>
            </div>
            <div className="timeline-panel"><div className="timeline-heading"><strong>Timeline preview</strong><span>Non-destructive markers only</span></div>
              {isVideo ? <>
                <input className="scrubber" type="range" aria-label="Scrub video" min={0} max={Math.max(duration, 0.01)} step={0.01} value={Math.min(playhead, duration || 0)} onChange={(e) => seek(Number(e.target.value))} />
                <div className="trim-tools"><button onClick={setInPoint}>Mark in: {formatTime(trimIn)}</button><button onClick={setOutPoint}>Mark out: {trimOut === null ? "end" : formatTime(trimOut)}</button><span>Preview markers only; exporting trims is not available yet.</span></div>
                <div className="timeline-clip"><span>▶</span><strong>{selected?.name}</strong></div>
              </> : <div className="timeline-empty">Select a video to see its clip timeline.</div>}
            </div>
          </section>

          <aside className="inspector editor-inspector">
            <div className="section-title"><div><h2>Media adjustments</h2><p>Preview changes without altering originals</p></div></div>
            {selected?.kind === "image" && !previewFailed ? <div className="adjustments">
              <h3>Image preview</h3>
              <label>Brightness <output>{brightness}%</output><input type="range" min="25" max="175" value={brightness} onChange={(event) => setBrightness(Number(event.target.value))} /></label>
              <label>Contrast <output>{contrast}%</output><input type="range" min="25" max="175" value={contrast} onChange={(event) => setContrast(Number(event.target.value))} /></label>
              <button className="secondary full" onClick={() => setRotation((value) => (value + 90) % 360)}>Rotate preview 90°</button>
              <button className="primary full" type="button" disabled={photoExportBusy}
                onClick={() => void exportAdjustedPhoto()}>
                {photoExportBusy ? "Saving adjusted PNG…" : "Save adjusted PNG copy"}
              </button>
              {photoExportNotice && <p role="status" className="photo-export-notice">{photoExportNotice}</p>}
              <p>Export to Pictures may sync with OneDrive. The original stays unchanged. PNG export does not preserve the source image's EXIF or ICC profile, and it is not saved into this project's media library.</p>
            </div> : <p className="model-hint">Select an image to adjust its preview, or choose Create to generate a new image.</p>}
            <button className="secondary full" onClick={() => { setStudioMode("create"); setStudioModeChosen(true); }}>Create an image</button>
            <GenerationHistory refreshVersion={historyVersion} onResult={addGenerated} />
          </aside>
          {message && <div className="notice" role="status"><span>{message}</span><button onClick={() => setMessage("")} aria-label="Dismiss notification">×</button></div>}
        </main>}

        {view === "models" && <main className="content-page"><div className="page-eyebrow">MODEL LIBRARY</div><h1>Find the right tool for your idea.</h1><p className="lead">Discover image and video models on Hugging Face, see what they do, what hardware they need and how they are licensed, and install the ones Optic Rift can run on this computer.</p>
          <ModelBrowser includeAdult={includeAdultModels} onLibraryChanged={() => setLibraryVersion((v) => v + 1)} />
          <HuggingFaceAccess />
          <div className="adult-preference">
            <div className="preference-top"><div><strong>Adult-oriented model discovery</strong><p>Models designed to create explicit sexual content will be labeled <b>Adult-focused (18+) · Explicit</b>. They will be excluded from discovery unless you opt in.</p></div><span className="adult-tag">18+</span></div>
            <label className="adult-toggle">
              <input type="checkbox" checked={includeAdultModels} onChange={(event) => {
                if (!event.target.checked) { setIncludeAdultModels(false); return; }
                const allowed = window.confirm("Adult-focused models can produce sexually explicit material. Confirm you are 18 or older and want to include their listings. Explicit previews will remain hidden until you choose to reveal them.");
                setIncludeAdultModels(allowed);
              }} />
              Include adult-focused models in discovery
            </label>
            <p className="preference-caption" role="status">{includeAdultModels ? "Opted in for this session. Explicit previews will still be hidden until opened intentionally." : "Off by default. This does not hide or alter media you've imported yourself."}</p>
            <p className="preference-caption">Session-only preference. Listings flagged "not for all audiences" by their source are shown with an 18+ badge; preview images are never loaded. This is not age verification, and unflagged models are not certified as general-audience.</p>
          </div>
          <p className="disclosure">Discovery is broad; execution is qualified. Search is anonymous. Installed models run in a separate local process with no network access.</p></main>}

        {view === "settings" && <main className="content-page"><div className="page-eyebrow">PREFERENCES</div><h1>Your studio stays on your computer.</h1><p className="lead">Local files, user-controlled AI models, and clear permission boundaries come first.</p><div className="setting-row"><span><strong>Application shell</strong><small>{status?.stage ?? "Browser preview: native Rust bridge unavailable"}</small></span><strong>{status ? "Tauri / Rust" : "Browser-only preview"}</strong></div><div className="setting-row"><span><strong>Model execution</strong><small>Local text-to-image through an isolated Python worker; set up on the Models page</small></span><strong>Text to image</strong></div><div className="setting-row"><span><strong>Persistent projects</strong><small>Named projects are saved locally and reopen after restart. Imported media is not saved into projects yet.</small></span><strong>Names only</strong></div><div className="setting-row"><span><strong>Local/cloud policy</strong><small>No cloud generation is enabled or silently selected</small></span><strong>Local first</strong></div></main>}
      </div>
    </div>
  );
}
