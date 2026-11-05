import { useEffect, useRef, useState, type ChangeEvent } from "react";
import { invoke } from "@tauri-apps/api/core";

type View = "studio" | "models" | "settings";
type MediaKind = "image" | "video";
type MediaItem = { id: string; name: string; kind: MediaKind; url: string; size: number };
type RuntimeStatus = { application: string; stage: string; generation_ready: boolean; project_storage_ready: boolean };

const formatTime = (seconds: number) => {
  const safe = Math.max(0, Number.isFinite(seconds) ? seconds : 0);
  const m = Math.floor(safe / 60);
  const s = Math.floor(safe % 60);
  return String(m).padStart(2, "0") + ":" + String(s).padStart(2, "0");
};
const compactSize = (size: number) => (size / (1024 * 1024)).toFixed(1) + " MB";

export default function App() {
  const [view, setView] = useState<View>("studio");
  const [items, setItems] = useState<MediaItem[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [prompt, setPrompt] = useState("");
  const [capability, setCapability] = useState("Text to video");
  const [message, setMessage] = useState("");
  const [brightness, setBrightness] = useState(100);
  const [contrast, setContrast] = useState(100);
  const [rotation, setRotation] = useState(0);
  const [duration, setDuration] = useState(0);
  const [playhead, setPlayhead] = useState(0);
  const [trimIn, setTrimIn] = useState(0);
  const [trimOut, setTrimOut] = useState<number | null>(null);
  const [status, setStatus] = useState<RuntimeStatus | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const objectUrls = useRef<string[]>([]);
  const selected = items.find((item) => item.id === selectedId) ?? null;
  const isVideo = selected?.kind === "video";

  useEffect(() => {
    void invoke<RuntimeStatus>("runtime_status")
      .then(setStatus)
      .catch(() => setStatus(null));
    return () => {
      objectUrls.current.forEach((url) => URL.revokeObjectURL(url));
      objectUrls.current = [];
    };
  }, []);

  function selectItem(id: string) {
    setSelectedId(id);
    setDuration(0);
    setPlayhead(0);
    setTrimIn(0);
    setTrimOut(null);
    setBrightness(100);
    setContrast(100);
    setRotation(0);
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
        <div className="brand"><span className="brand-icon">◉</span><span>OPTIC <b>RIFT</b><small>VISUAL STUDIO</small></span></div>
        <div className="nav-group">
          <div className="group-label">WORKSPACE</div>
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
          <div className="breadcrumbs">OPTIC RIFT <span>/</span> {view === "studio" ? "Creative studio" : view === "models" ? "Model library" : "Preferences"}</div>
          <div className="top-actions"><span className="pill">PRE-ALPHA</span><button className="top-button" onClick={() => setView("models")}>Model manager</button></div>
        </header>

        {view === "studio" && <main className="workspace">
          <section className="media-panel">
            <div className="section-title"><div><h2>Media library</h2><p>Your images and clips</p></div><span className="count">{items.length}</span></div>
            <input ref={inputRef} type="file" accept="image/*,video/*" multiple hidden onChange={addMedia} aria-label="Select image and video files" />
            <button className="import-button" onClick={() => inputRef.current?.click()}>＋ Import media</button>
            <div className="media-list">
              {items.length === 0 && <div className="empty-library">Your canvas starts here.<br /><span>Import a video or image to preview it.</span></div>}
              {items.map((item) => <div className={selectedId === item.id ? "media-row active" : "media-row"} key={item.id}>
                <button onClick={() => selectItem(item.id)} className="media-pick">
                  <span className="media-kind">{item.kind === "video" ? "▶" : "▧"}</span><span className="media-label"><strong title={item.name}>{item.name}</strong><small>{item.kind} · {compactSize(item.size)}</small></span>
                </button>
                <button className="remove-item" aria-label={"Remove " + item.name} title="Remove from this session" onClick={() => removeItem(item.id)}>×</button>
              </div>)}
            </div>
            <div className="sidebar-note">Imported files stay on your computer. This preview does not save projects across restarts.</div>
          </section>

          <section className="studio-center">
            <div className="canvas-header"><div><strong>Preview</strong><span>{selected ? selected.name : "No media selected"}</span></div><span className="preview-label">{selected?.kind ?? "READY"}</span></div>
            <div className="canvas">
              {selected?.kind === "image" && <img key={selected.id} src={selected.url} alt={selected.name} style={{ filter: "brightness(" + brightness + "%) contrast(" + contrast + "%)", transform: "rotate(" + rotation + "deg)" }} />}
              {selected?.kind === "video" && <video key={selected.id} ref={videoRef} src={selected.url} controls preload="metadata" playsInline onLoadedMetadata={(event) => { const length = event.currentTarget.duration; setDuration(Number.isFinite(length) ? length : 0); setPlayhead(0); }} onTimeUpdate={(event) => setPlayhead(event.currentTarget.currentTime)} onError={() => setMessage("This video's codec may not be supported by the preview. Native proxy handling is planned.")} />}
              {!selected && <div className="empty-canvas"><div className="empty-icon">◉</div><h1>Make something extraordinary.</h1><p>Start with a photo or video. Explore AI generation as compatible models become available.</p><button className="primary" onClick={() => inputRef.current?.click()}>Import your first clip</button></div>}
            </div>
            <div className="player-controls">
              {isVideo ? <><button className="control" onClick={() => seek(playhead - 0.1)} title="Step back 0.1 seconds">↶ 0.1s</button><button className="control" onClick={() => seek(playhead + 0.1)} title="Step forward 0.1 seconds">0.1s ↷</button><span className="time">{formatTime(playhead)} / {formatTime(duration)}</span></> :
                <span className="time">{selected ? "Image preview" : "Import media to begin"}</span>}
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

          <aside className="inspector">
            <div className="section-title"><div><h2>Creation tools</h2><p>Ideas to visuals</p></div></div>
            <label className="field-label" htmlFor="capability">What would you like to make?</label>
            <select id="capability" value={capability} onChange={(event) => setCapability(event.target.value)}>
              <option>Text to video</option><option>Image to video</option><option>Text to image</option><option>Image to image</option><option>Inpainting</option><option>Upscaling</option>
            </select>
            <label className="field-label" htmlFor="prompt">Describe your idea</label>
            <textarea id="prompt" value={prompt} onChange={(event) => setPrompt(event.target.value)} placeholder="A quiet mountain lake at sunrise, soft golden light and a gentle breeze..." rows={6} />
            <div className="model-info"><strong>No model connected</strong><p>We will discover models through Hugging Face and clearly show compatibility and license terms before download.</p></div>
            <button className="primary full" disabled>Generate · Engine required</button>
            <button className="secondary full" onClick={() => setView("models")}>Explore model setup</button>
            {selected?.kind === "image" && <div className="adjustments">
              <h3>Image preview adjustments</h3>
              <label>Brightness <output>{brightness}%</output><input type="range" min="25" max="175" value={brightness} onChange={(event) => setBrightness(Number(event.target.value))} /></label>
              <label>Contrast <output>{contrast}%</output><input type="range" min="25" max="175" value={contrast} onChange={(event) => setContrast(Number(event.target.value))} /></label>
              <button className="secondary full" onClick={() => setRotation((value) => (value + 90) % 360)}>Rotate preview 90°</button>
              <p>Preview only. Saving adjusted images will require a native export pipeline.</p>
            </div>}
          </aside>
          {message && <div className="notice" role="status"><span>{message}</span><button onClick={() => setMessage("")} aria-label="Dismiss notification">×</button></div>}
        </main>}

        {view === "models" && <main className="content-page"><div className="page-eyebrow">MODEL LIBRARY</div><h1>Find the right tool for your idea.</h1><p className="lead">Optic Rift will discover image and video models through Hugging Face, then explain what they do, what hardware they need, and how their licenses work.</p><div className="feature-grid"><div className="feature"><span className="feature-icon">⌕</span><h3>Discover by capability</h3><p>Browse text-to-image, image-to-video, video-to-video, inpainting, restoration, and more.</p><strong>Planned</strong></div><div className="feature"><span className="feature-icon">◈</span><h3>Know before you download</h3><p>Review gating, weights, licenses, disk requirements, and your hardware fit.</p><strong>Planned</strong></div><div className="feature"><span className="feature-icon">▣</span><h3>Install only what you want</h3><p>Separate model dependencies, explicit consent, and measured worker readiness.</p><strong>Planned</strong></div></div><p className="disclosure">Model discovery and execution are not connected in this pre-alpha shell. No Hugging Face token is requested or stored.</p></main>}

        {view === "settings" && <main className="content-page"><div className="page-eyebrow">PREFERENCES</div><h1>Your studio stays on your computer.</h1><p className="lead">Local files, user-controlled AI models, and clear permission boundaries come first.</p><div className="setting-row"><span><strong>Application shell</strong><small>{status?.stage ?? "Browser preview: native Rust bridge unavailable"}</small></span><strong>{status ? "Tauri / Rust" : "Browser-only preview"}</strong></div><div className="setting-row"><span><strong>Model execution</strong><small>Independent Python engines; none connected yet</small></span><strong>Not ready</strong></div><div className="setting-row"><span><strong>Persistent projects</strong><small>Native non-destructive media/project storage is planned</small></span><strong>Not ready</strong></div><div className="setting-row"><span><strong>Local/cloud policy</strong><small>No cloud generation is enabled or silently selected</small></span><strong>Local first</strong></div></main>}
      </div>
    </div>
  );
}
