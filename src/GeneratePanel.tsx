import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SIZE_PRESETS, settingsError, type GpuCheck, type GpuDecision, type ModelLibrary, type Provenance } from "./models";
import { saveWithPicturesConsent } from "./exportPrivacy";
import { imageToImageStepError, localImageSourceChoices, type LocalImageSource } from "./imageToImage";
import GenerationFilmstrip from "./GenerationFilmstrip";

const errorText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : "The desktop bridge is unavailable.");

type Props = {
  prompt: string;
  mode: "text-to-image" | "image-to-image";
  layout?: "inspector" | "create";
  /** Create layout only: false while the Create workspace is hidden. */
  visible?: boolean;
  onPromptChange?: (text: string) => void;
  onModeChange?: (mode: "text-to-image" | "image-to-image") => void;
  historyVersion: number;
  projectId: string | null;
  libraryVersion: number;
  onOpenModels: () => void;
  onResult: (result: { name: string; url: string; size: number }) => void;
  onGenerated: () => void;
};

type WorkerEvent = { event: string; job_id?: string; step?: number; total?: number; seconds?: number };

export default function GeneratePanel({ prompt, mode, layout = "inspector", visible = true, onPromptChange, onModeChange, historyVersion, projectId, libraryVersion, onOpenModels, onResult, onGenerated }: Props) {
  const [library, setLibrary] = useState<ModelLibrary | null>(null);
  const [model, setModel] = useState("");
  const [negative, setNegative] = useState("");
  const [preset, setPreset] = useState(0);
  const [steps, setSteps] = useState(25);
  const [guidance, setGuidance] = useState(7);
  const [seedText, setSeedText] = useState("");
  const [jobId, setJobId] = useState<string | null>(null);
  const [status, setStatus] = useState("");
  const [progress, setProgress] = useState<{ step: number; total: number } | null>(null);
  const [error, setError] = useState("");
  const [result, setResult] = useState<{ provenance: Provenance; url: string } | null>(null);
  const [saved, setSaved] = useState("");
  const [sources, setSources] = useState<LocalImageSource[]>([]);
  const [sourceId, setSourceId] = useState("");
  const [sourceError, setSourceError] = useState("");
  const [strength, setStrength] = useState(0.65);
  const [confirm, setConfirm] = useState<Extract<GpuDecision, { kind: "confirm" }> | null>(null);
  const [checking, setChecking] = useState(false);
  const [preview, setPreview] = useState<{ record: Provenance; uri: string } | null>(null);
  const [sourceUri, setSourceUri] = useState<string | null>(null);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [cancelling, setCancelling] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const lib = await invoke<ModelLibrary>("models_library");
      setLibrary(lib);
      const usable = lib.installed.filter((m) => m.files_present && m.manifest.adapter.endsWith("text-to-image"));
      setModel((current) => (usable.some((m) => `${m.manifest.repo}@${m.manifest.revision}` === current) ? current : usable[0] ? `${usable[0].manifest.repo}@${usable[0].manifest.revision}` : ""));
    } catch {
      setLibrary(null);
    }
  }, []);
  useEffect(() => { void refresh(); }, [refresh, libraryVersion]);

  const refreshSources = useCallback(async () => {
    try {
      const records = await invoke<Provenance[]>("generation_outputs");
      const valid = localImageSourceChoices(records);
      setSources(valid);
      setSourceId((current) => valid.some((p) => p.id === current) ? current : (layout === "create" ? "" : valid[0]?.id ?? ""));
      setSourceError("");
    } catch (e) {
      setSources([]);
      setSourceId("");
      setSourceError(errorText(e));
    }
  }, [layout]);
  useEffect(() => { if (mode === "image-to-image") void refreshSources(); }, [mode, historyVersion, refreshSources]);

  useEffect(() => {
    if (!jobId) return;
    const stop = listen<WorkerEvent>("generation", (e) => {
      const p = e.payload;
      if (p.event === "verifying-model" && p.job_id === jobId) setStatus("Checking installed model files before loading…");
      if (p.event === "loading" && p.job_id === jobId) setStatus("Loading the model onto the GPU…");
      if (p.event === "loaded") setStatus(`Model loaded${p.seconds ? ` in ${p.seconds}s` : ""}. Generating…`);
      if (p.event === "progress" && p.job_id === jobId) {
        setProgress({ step: p.step ?? 0, total: p.total ?? 1 });
        setStatus(`Generating · step ${p.step} of ${p.total}`);
      }
      if (p.event === "worker-exited") setStatus("The image worker stopped.");
      if (p.event === "auto-unloaded") { setStatus("Model unloaded after 5 idle minutes to free graphics memory for other apps."); void refresh(); }
    });
    return () => { void stop.then((f) => f()); };
  }, [jobId]);

  useEffect(() => {
    setStatus("");
    setError("");
    setPreview(null);
  }, [mode]);

  useEffect(() => {
    if (layout !== "create" || mode !== "image-to-image" || !sourceId) {
      setSourceUri(null);
      return;
    }
    let active = true;
    setSourceUri(null);
    void invoke<string>("generation_output_image", { jobId: sourceId })
      .then((uri) => { if (active && uri.startsWith("data:image/png;base64,")) setSourceUri(uri); })
      .catch((e) => { if (active) setError(errorText(e)); });
    return () => { active = false; };
  }, [layout, mode, sourceId]);

  const onGalleryError = useCallback((message: string) => setError(message), []);
  const onGallerySelect = useCallback((record: Provenance, uri: string) => {
    setPreview({ record, uri });
    if (mode === "image-to-image") setSourceId(record.job_id);
  }, [mode]);

  const usable = library?.installed.filter((m) => m.files_present && m.manifest.adapter.endsWith("text-to-image")) ?? [];
  const source = sources.find((p) => p.id === sourceId);
  const size = mode === "image-to-image" && source
    ? { width: source.width, height: source.height }
    : SIZE_PRESETS[preset];
  const missingSource = mode === "image-to-image" && !source;
  const seed = seedText.trim() === "" ? null : Number(seedText);
  const problem = settingsError(prompt, { width: size.width, height: size.height, steps, guidance, seed })
    || imageToImageStepError(mode, steps, strength);
  const runtimeReady = Boolean(library?.runtime.installed);

  /** Check who else is using the GPU first; ask before sharing it with another app. */
  async function requestGenerate() {
    const [repo, revision] = model.split("@");
    setError(""); setConfirm(null); setChecking(true);
    try {
      const check = await invoke<GpuCheck>("gpu_check", {
        repo, revision, width: size.width, height: size.height, steps, mode,
        sourceJobId: mode === "image-to-image" ? sourceId : null,
        strength: mode === "image-to-image" ? strength : null,
      });
      if (check.decision.kind === "block") setError(check.decision.message);
      else if (check.decision.kind === "confirm") setConfirm(check.decision);
      else await generate([]);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setChecking(false);
    }
  }

  async function generate(acknowledged: number[]) {
    const [repo, revision] = model.split("@");
    const id = crypto.randomUUID();
    setConfirm(null);
    setJobId(id); setError(""); setSaved(""); setStatus("Starting…"); setProgress(null);
    const started = performance.now();
    try {
      const provenance = await invoke<Provenance>("generation_generate", { request: {
        job_id: id, repo, revision, prompt, negative_prompt: negative, width: size.width, height: size.height,
        steps, guidance, seed, project_id: projectId, gpu_acknowledged: acknowledged,
        mode, source_job_id: mode === "image-to-image" ? sourceId : null,
        strength: mode === "image-to-image" ? strength : null,
      } });
      const url = await invoke<string>("generation_output_image", { jobId: id });
      setResult({ provenance, url });
      setPreview(null);
      onGenerated();
      if (mode === "image-to-image") void refreshSources();
      setStatus(`Done in ${((performance.now() - started) / 1000).toFixed(1)}s · seed ${provenance.parameters.seed}`);
      void refresh();
    } catch (e) {
      const text = errorText(e);
      // Another app started using the GPU between the check and the job: ask again.
      if (text.startsWith("GPU_BUSY:")) void requestGenerate();
      else setError(text);
      setStatus("");
    } finally {
      setJobId(null);
      setProgress(null);
      setCancelling(false);
    }
  }

  async function save() {
    if (!result) return;
    try {
      const path = await saveWithPicturesConsent(
        result.provenance.job_id,
        (warning) => window.confirm(warning),
        (jobId) => invoke<string>("generation_save", { jobId }),
      );
      if (path !== null) setSaved(`Saved to ${path}`);
    } catch (e) {
      setSaved(errorText(e));
    }
  }

  async function addToMedia() {
    if (!result) return;
    const blob = await (await fetch(result.url)).blob();
    onResult({ name: `generated-${result.provenance.job_id.slice(0, 8)}.png`, url: URL.createObjectURL(blob), size: blob.size });
  }

  if (layout === "create") {
    const hasModel = usable.length > 0 && runtimeReady;
    const latestResult = result?.provenance.parameters.mode === mode
      && (mode === "text-to-image" || result?.provenance.source?.job_id === sourceId)
      ? result : null;
    const selected = mode === "text-to-image"
      ? (preview ?? (latestResult ? { record: latestResult.provenance, uri: latestResult.url } : null))
      : null;
    const active = mode === "image-to-image" ? latestResult && { record: latestResult.provenance, uri: latestResult.url } : selected;

    async function stageSave() {
      const record = active?.record;
      if (!record) return;
      setSaved("");
      try {
        const path = await saveWithPicturesConsent(
          record.job_id,
          (warning) => window.confirm(warning),
          (id) => invoke<string>("generation_save", { jobId: id }),
        );
        if (path !== null) setSaved(`Saved to ${path}`);
      } catch (e) { setSaved(errorText(e)); }
    }

    async function stageEdit() {
      if (!active) return;
      try {
        const response = await fetch(active.uri);
        const blob = await response.blob();
        if (blob.type !== "image/png" || !blob.size) throw new Error("The result is not a valid PNG.");
        onResult({ name: `generated-${active.record.job_id.slice(0, 8)}.png`, url: URL.createObjectURL(blob), size: blob.size });
      } catch (e) { setError(errorText(e)); }
    }

    return <div className="create-panel">
      <section className="create-stage" aria-label="Generation preview">
        {mode === "image-to-image" ? <div className="create-compare">
          <div className="create-preview"><span className="create-preview-label">Starting image</span>
            {sourceUri ? <img src={sourceUri} alt="Original starting image, unchanged" /> : <p>Choose a starting image from Recent generations.</p>}
          </div>
          <div className="create-preview"><span className="create-preview-label">Result</span>
            {active ? <img src={active.uri} alt="Transformed image" /> : <p>{jobId ? "Transforming your image…" : "The transformed image will appear here."}</p>}
          </div>
        </div> : <div className="create-preview create-preview-single">
          {active ? <img src={active.uri} alt="Generated image" /> : <div className="create-welcome">
            <strong>{hasModel ? "Your next image starts here." : "Set up an image model to start creating."}</strong>
            <p>{hasModel ? "Enter a prompt below and generate locally." : "Open Models to install a compatible model and its local runtime."}</p>
            {!hasModel && <button className="secondary" onClick={onOpenModels}>Open Models</button>}
          </div>}
        </div>}
        {jobId && <div className="create-progress" role="status" aria-live="polite">
          <strong>{cancelling ? "Cancelling…" : status || "Generating locally…"}</strong>
          <progress value={progress?.step ?? 0} max={progress?.total || 1} />
        </div>}
        <div className="create-stage-footer">
          <span role="status">{status || (active ? `Seed ${active.record.parameters.seed} · ${active.record.output.width}×${active.record.output.height}` : "Local generation. No cloud inference.")}</span>
          <div className="model-actions">
            {jobId && <button className="secondary" disabled={cancelling} onClick={() => {
              setCancelling(true);
              void invoke("generation_cancel", { jobId }).catch((e) => setError(errorText(e)));
            }}>Cancel</button>}
            {active && <><button className="secondary" onClick={() => void stageEdit()}>Open in Edit media</button>
              <button className="secondary" onClick={() => void stageSave()}>Save copy (may sync)</button></>}
          </div>
        </div>
        {confirm && <div className="create-gpu-consent gpu-confirm" role="alertdialog" aria-modal="true" aria-labelledby="create-gpu-title" aria-describedby="create-gpu-message"
          onKeyDown={(e) => { if (e.key === "Escape") setConfirm(null); }}>
          <strong id="create-gpu-title">Your GPU is in use</strong><p id="create-gpu-message">{confirm.message}</p>
          {confirm.apps.length > 0 && <ul>{confirm.apps.map((a) => <li key={a.pid}>{a.name} · {(a.dedicated_mb / 1024).toFixed(1)} GB</li>)}</ul>}
          {/* The Generate button is disabled while this is open, so move keyboard focus here (safe default). */}
          <div className="model-actions"><button className="secondary" autoFocus onClick={() => setConfirm(null)}>Not now</button>
            <button className="primary" onClick={() => void generate(confirm.apps.map((a) => a.pid))}>Generate anyway</button></div>
        </div>}
        {error && <p className="project-error create-stage-alert" role="alert">{error}</p>}
        {saved && <p className="model-hint create-stage-alert" role="status">{saved}</p>}
      </section>

      <section className="create-prompt" aria-label="Generation prompt">
        <label htmlFor="create-prompt">Describe {mode === "image-to-image" ? "the change" : "your idea"}</label>
        <div className="create-prompt-row">
          <textarea id="create-prompt" value={prompt} onChange={(e) => onPromptChange?.(e.target.value)}
            maxLength={2000} rows={2} placeholder={mode === "image-to-image" ? "Describe how the image should change…" : "Describe what you want to create…"} />
          <button className="primary" type="button" disabled={!hasModel || jobId !== null || checking || confirm !== null || Boolean(problem) || !model || missingSource}
            onClick={() => void requestGenerate()}>{jobId ? "Generating…" : checking ? "Checking GPU…" : mode === "image-to-image" ? "Transform" : "Generate"}</button>
        </div>
        {/* An untouched, empty prompt is a starting state, not an error. */}
        {problem && prompt.trim() ? <small className="project-error" role="status">{problem}</small> : null}
      </section>

      <GenerationFilmstrip refreshVersion={historyVersion} selectedId={mode === "image-to-image" ? sourceId : selected?.record.job_id ?? null}
        onSelect={onGallerySelect} onError={onGalleryError} active={visible} />

      <aside className="create-settings" aria-label="Image generation settings">
        <h2>Generation settings</h2>
        <div className="create-mode-switch" role="group" aria-label="Generation mode">
          <button type="button" className={mode === "text-to-image" ? "selected" : ""} disabled={jobId !== null} onClick={() => onModeChange?.("text-to-image")}>Text to image</button>
          <button type="button" className={mode === "image-to-image" ? "selected" : ""} disabled={jobId !== null} onClick={() => onModeChange?.("image-to-image")}>Image to image</button>
        </div>
        <label htmlFor="create-model">Model</label>
        <select id="create-model" value={model} onChange={(e) => setModel(e.target.value)} disabled={jobId !== null}>
          {usable.length === 0 && <option value="">No compatible local model</option>}
          {usable.map((item) => <option key={item.dir} value={`${item.manifest.repo}@${item.manifest.revision}`}>{item.manifest.repo} · {item.manifest.revision.slice(0, 8)}</option>)}
        </select>
        {mode === "image-to-image" && <>
          <label htmlFor="create-source">Starting image</label>
          <select id="create-source" value={sourceId} disabled={jobId !== null || checking} onChange={(e) => setSourceId(e.target.value)}>
            {/* Without an explicit empty choice the browser would display the first history item as selected. */}
            <option value="">{sources.length ? "Choose a starting image…" : "No saved generations yet"}</option>
            {sources.map((item) => <option key={item.id} value={item.id}>{item.id.slice(0, 8)} · {item.width}×{item.height}</option>)}
          </select>
          <label htmlFor="create-strength">Change strength: {strength.toFixed(2)}</label>
          <input id="create-strength" type="range" min={0.05} max={0.95} step={0.05} value={strength} disabled={jobId !== null} onChange={(e) => setStrength(Number(e.target.value))} />
          <small>Lower values preserve more of the starting image. The original is never modified.</small>
          {sourceError && <p className="project-error" role="alert">{sourceError}</p>}
        </>}
        <label htmlFor="create-negative">Avoid (optional)</label>
        <input id="create-negative" value={negative} maxLength={2000} onChange={(e) => setNegative(e.target.value)} placeholder="blurry, low quality" />
        {mode === "text-to-image" ? <>
          <label htmlFor="create-size">Image size</label>
          <select id="create-size" value={preset} onChange={(e) => setPreset(Number(e.target.value))}>
            {SIZE_PRESETS.map((item, index) => <option key={item.label} value={index}>{item.label}</option>)}
          </select>
        </> : <small>Output follows the starting image size: {source ? `${source.width}×${source.height}` : "choose an image"}.</small>}
        <button type="button" className="secondary" aria-expanded={advancedOpen} onClick={() => setAdvancedOpen((v) => !v)}>Advanced settings {advancedOpen ? "▴" : "▾"}</button>
        {advancedOpen && <div className="create-advanced">
          <label>Steps<input type="number" min={1} max={100} value={steps} onChange={(e) => setSteps(Number(e.target.value))} /></label>
          <label>Guidance<input type="number" min={0} max={20} step={0.5} value={guidance} onChange={(e) => setGuidance(Number(e.target.value))} /></label>
          <label>Seed<input value={seedText} onChange={(e) => setSeedText(e.target.value.replace(/[^0-9]/g, ""))} placeholder="random" /></label>
        </div>}
        {library?.loaded && !jobId && <button className="secondary" onClick={() => void invoke("generation_unload").then(refresh).catch((e) => setError(errorText(e)))}>Unload model from GPU</button>}
        <p className="model-hint">Results and prompt metadata stay in the app's local output folder. This is not an encrypted vault. Saving to Pictures may sync through OneDrive; Optic Rift asks before export.</p>
      </aside>
    </div>;
  }

  if (library && usable.length === 0) {
    return <div className="model-info"><strong>No image model installed</strong><p>Install a supported text-to-image model to generate locally on your GPU.</p><button className="secondary full" onClick={onOpenModels}>Find a model</button></div>;
  }
  if (library && !runtimeReady) {
    return <div className="model-info"><strong>Image runtime not installed</strong><p>Models are installed, but the local runtime that runs them is not.</p><button className="secondary full" onClick={onOpenModels}>Set up the runtime</button></div>;
  }

  return (
    <div className="generate-panel">
      <label className="field-label" htmlFor="gen-model">Model</label>
      <select id="gen-model" value={model} onChange={(e) => setModel(e.target.value)} disabled={jobId !== null}>
        {usable.map((m) => <option key={m.dir} value={`${m.manifest.repo}@${m.manifest.revision}`}>{m.manifest.repo} · {m.manifest.revision.slice(0, 8)}</option>)}
      </select>
      {mode === "image-to-image" && <>
        <label className="field-label" htmlFor="gen-source">Starting image (previously generated here)</label>
        <select id="gen-source" value={sourceId} disabled={jobId !== null || checking} onChange={(e) => setSourceId(e.target.value)}>
          {!sources.length && <option value="">No saved local generations</option>}
          {sources.map((item) => <option key={item.id} value={item.id}>
            {item.label} · {item.width}×{item.height} · {item.id.slice(0, 8)}
          </option>)}
        </select>
        <button className="secondary" type="button" disabled={jobId !== null || checking} onClick={() => void refreshSources()}>Refresh starting images</button>
        <label className="field-label" htmlFor="gen-strength">Change strength: {strength.toFixed(2)}</label>
        <input id="gen-strength" type="range" min={0.05} max={0.95} step={0.05} value={strength} disabled={jobId !== null} onChange={(e) => setStrength(Number(e.target.value))} />
        <small className="model-hint">Lower values preserve more of the image. The original stays unchanged; results are new local outputs, not project assets.</small>
        {sourceError && <p className="project-error" role="alert">{sourceError}</p>}
      </>}
      <label className="field-label" htmlFor="gen-negative">Avoid (optional)</label>
      <input id="gen-negative" value={negative} maxLength={2000} onChange={(e) => setNegative(e.target.value)} placeholder="blurry, low quality" />
      {mode === "text-to-image" ? <>
        <label className="field-label" htmlFor="gen-size">Size</label>
        <select id="gen-size" value={preset} onChange={(e) => setPreset(Number(e.target.value))}>{SIZE_PRESETS.map((p, i) => <option key={p.label} value={i}>{p.label}</option>)}</select>
      </> : <p className="model-hint">Output size follows the starting image: {source ? `${size.width} × ${size.height}` : "choose an image first"}.</p>}
      <div className="gen-row">
        <label>Steps <input type="number" min={1} max={100} value={steps} onChange={(e) => setSteps(Number(e.target.value))} /></label>
        <label>Guidance <input type="number" min={0} max={20} step={0.5} value={guidance} onChange={(e) => setGuidance(Number(e.target.value))} /></label>
        <label>Seed <input value={seedText} onChange={(e) => setSeedText(e.target.value.replace(/[^0-9]/g, ""))} placeholder="random" /></label>
      </div>
      {problem && prompt.trim() && <p className="project-error">{problem}</p>}
      {missingSource && <p className="model-hint">Generate an image first, then select it here to transform.</p>}
      <button className="primary full" disabled={jobId !== null || checking || confirm !== null || Boolean(problem) || !model || missingSource} onClick={() => void requestGenerate()}>{jobId ? "Generating…" : checking ? "Checking GPU…" : mode === "image-to-image" ? "Transform locally" : "Generate locally"}</button>
      {confirm && (
        <div className="gpu-confirm" role="alertdialog" aria-label="Another app is using the GPU">
          <strong>Your GPU is in use</strong>
          <p>{confirm.message}</p>
          {confirm.apps.length > 0 && <ul>{confirm.apps.map((a) => <li key={a.pid}>{a.name} · {(a.dedicated_mb / 1024).toFixed(1)} GB · {a.engine_3d_percent.toFixed(0)}% 3D</li>)}</ul>}
          <div className="model-actions">
            <button className="secondary" onClick={() => setConfirm(null)}>Not now</button>
            <button className="primary" onClick={() => void generate(confirm.apps.map((a) => a.pid))}>Generate anyway</button>
          </div>
        </div>
      )}
      {jobId && <button className="secondary full" onClick={() => void invoke("generation_cancel", { jobId })}>Cancel</button>}
      {status && <p className="model-hint" role="status">{status}</p>}
      {error && <p className="project-error" role="alert">{error}</p>}
      {result && (
        <div className="gen-result">
          <img src={result.url} alt={`Generated: ${result.provenance.prompt}`} />
          <small>{result.provenance.model.repo} · {result.provenance.parameters.width}×{result.provenance.parameters.height} · {result.provenance.parameters.steps} steps · seed {result.provenance.parameters.seed} · {result.provenance.metrics.generation_seconds}s · peak {result.provenance.metrics.peak_vram_allocated_mb} MB VRAM · sha256 {result.provenance.output.sha256.slice(0, 12)}…</small>
          <div className="model-actions"><button className="secondary" onClick={() => void save()}>Save to Pictures (may sync)</button><button className="secondary" onClick={() => void addToMedia()}>Open in studio</button></div>
          <small>Saving to Pictures may upload your image and prompt metadata if that folder is synced to OneDrive or another cloud service. You will be asked first.</small>
          {saved && <small role="status">{saved}</small>}
          <small>{result.provenance.lineage_note}</small>
        </div>
      )}
      {library?.loaded && !jobId && <button className="link-button" onClick={() => void invoke("generation_unload").then(refresh).catch((e) => setError(errorText(e)))}>Unload model from GPU</button>}
    </div>
  );
}
