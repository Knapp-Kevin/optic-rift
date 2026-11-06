import { useCallback, useEffect, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  compatibilityText, formatBytes, licenseAcknowledgement, licenseText, modelStates,
  type Capability, type CapabilityInfo, type InstallEvent, type Inspection, type ModelLibrary, type ModelSummary, type SearchResult,
} from "./models";

const errorText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : "The desktop bridge is unavailable.");

type Props = { includeAdult: boolean; onLibraryChanged: () => void };

export default function ModelBrowser({ includeAdult, onLibraryChanged }: Props) {
  const [capabilities, setCapabilities] = useState<CapabilityInfo[]>([]);
  const [capability, setCapability] = useState<Capability>("text-to-image");
  const [query, setQuery] = useState("");
  const [result, setResult] = useState<SearchResult | null>(null);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState("");
  const [inspection, setInspection] = useState<Inspection | null>(null);
  const [inspecting, setInspecting] = useState<string | null>(null);
  const [inspectError, setInspectError] = useState("");
  const [library, setLibrary] = useState<ModelLibrary | null>(null);

  const refreshLibrary = useCallback(async () => {
    try { setLibrary(await invoke<ModelLibrary>("models_library")); } catch { setLibrary(null); }
  }, []);

  useEffect(() => {
    void invoke<CapabilityInfo[]>("catalog_capabilities").then(setCapabilities).catch(() => setCapabilities([]));
    void refreshLibrary();
  }, [refreshLibrary]);

  async function search(event?: FormEvent) {
    event?.preventDefault();
    setSearching(true);
    setSearchError("");
    try {
      setResult(await invoke<SearchResult>("catalog_search", { query, capability, includeAdult }));
    } catch (e) {
      setSearchError(errorText(e));
    } finally {
      setSearching(false);
    }
  }

  async function inspect(model: ModelSummary | { id: string }, revision?: string) {
    setInspecting(model.id);
    setInspectError("");
    try {
      setInspection(await invoke<Inspection>("catalog_inspect", { repo: model.id, revision: revision ?? null, capability }));
    } catch (e) {
      setInspection(null);
      setInspectError(errorText(e));
    } finally {
      setInspecting(null);
    }
  }

  const adaptersFor = capabilities.find((c) => c.id === capability)?.adapters ?? [];

  return (
    <section className="model-browser" aria-label="Hugging Face model browser">
      <RuntimePanel library={library} onChanged={() => { void refreshLibrary(); onLibraryChanged(); }} />
      <InstalledPanel library={library} onChanged={() => { void refreshLibrary(); onLibraryChanged(); }} onInspect={(repo, revision) => void inspect({ id: repo }, revision)} />

      <h2 className="model-section-title">Discover models on Hugging Face</h2>
      <form className="model-search" onSubmit={search}>
        <select aria-label="Capability" value={capability} onChange={(e) => { setCapability(e.target.value as Capability); setResult(null); setInspection(null); }}>
          {capabilities.map((c) => <option key={c.id} value={c.id}>{c.label}{c.adapters.length ? "" : " (discover only)"}</option>)}
        </select>
        <input aria-label="Search models" value={query} maxLength={120} onChange={(e) => setQuery(e.target.value)} placeholder="Search by name, e.g. SDXL, SSD-1B, upscaler" />
        <button className="primary" type="submit" disabled={searching}>{searching ? "Searching…" : "Search"}</button>
      </form>
      <p className="model-hint">
        {adaptersFor.length
          ? <>Optic Rift can run: {adaptersFor.join("; ")}. Other results are listed but won't install.</>
          : <>No Optic Rift adapter runs this capability yet. You can browse and inspect; public video models may allow caching a small configuration sample (not weights).</>}
        {" "}Search is anonymous; model cards and preview images are not loaded.
      </p>
      {searchError && <p className="project-error" role="alert">{searchError}</p>}
      {result && <p className="model-hint">{result.models.length} results{result.hidden_not_for_all_audiences ? ` · ${result.hidden_not_for_all_audiences} flagged "not for all audiences" hidden (adult discovery is off)` : ""}</p>}

      <div className="model-columns">
        <ul className="model-results">
          {result?.models.map((m) => (
            <li key={m.id} className={inspection?.detail.id === m.id ? "model-result active" : "model-result"}>
              <button onClick={() => void inspect(m)} disabled={inspecting !== null}>
                <strong>{m.id}</strong>
                <span className="model-badges">
                  <span className="badge">{m.format}</span>
                  <span className="badge">license: {m.license ?? "unknown"}</span>
                  {m.gated !== "no" && <span className="badge warn">gated</span>}
                  {m.audience === "not-for-all-audiences" && <span className="badge adult">18+ flagged by source</span>}
                  {library?.installed.some((i) => i.manifest.repo === m.id) && <span className="badge ok">installed</span>}
                </span>
                <small>{compatibilityText(m.compatibility)} · {m.downloads.toLocaleString()} downloads</small>
              </button>
            </li>
          ))}
        </ul>
        <div className="model-detail">
          {inspecting && <p className="model-hint">Inspecting {inspecting} at its current revision…</p>}
          {inspectError && <p className="project-error" role="alert">{inspectError}</p>}
          {inspection && !inspecting && <ModelDetailView inspection={inspection} library={library} onInstalled={() => { void refreshLibrary(); onLibraryChanged(); void inspect({ id: inspection.detail.id }, inspection.detail.revision); }} />}
          {!inspection && !inspecting && !inspectError && <p className="model-hint">Select a model to see its license, files, hardware needs and whether Optic Rift can run it.</p>}
        </div>
      </div>
    </section>
  );
}

function StateRow({ inspection, loaded }: { inspection: Inspection; loaded: [string, string] | null }) {
  const states = modelStates(inspection, loaded);
  const order = [["Discovered", states.discovered], ["Supported", states.supported], ["Verified", states.verified], ["Installed", states.installed], ["Ready", states.ready]] as const;
  return (
    <ol className="model-states">
      {order.map(([label, s]) => <li key={label} className={s.reached ? "reached" : ""} title={s.detail}><strong>{s.reached ? "✓" : "○"} {label}</strong><small>{s.detail}</small></li>)}
    </ol>
  );
}

function ModelDetailView({ inspection, library, onInstalled }: { inspection: Inspection; library: ModelLibrary | null; onInstalled: () => void }) {
  const { detail, installed, gpu, free_bytes, model_dir } = inspection;
  const plan = detail.plans[0] ?? null;
  const [licenseOk, setLicenseOk] = useState(false);
  const [downloadOk, setDownloadOk] = useState(false);
  const [installId, setInstallId] = useState<string | null>(null);
  const [progress, setProgress] = useState<InstallEvent | null>(null);
  const [error, setError] = useState("");

  useEffect(() => { setLicenseOk(false); setDownloadOk(false); setError(""); setProgress(null); }, [detail.id, detail.revision]);
  useEffect(() => {
    if (!installId) return;
    const stop = listen<InstallEvent>("model-install", (e) => { if (e.payload.install_id === installId) setProgress(e.payload); });
    return () => { void stop.then((f) => f()); };
  }, [installId]);

  async function install() {
    if (!plan) return;
    const id = crypto.randomUUID();
    setInstallId(id);
    setError("");
    try {
      await invoke("model_install", { request: { install_id: id, repo: detail.id, revision: detail.revision, adapter: plan.plan.adapter, license_acknowledged: licenseAcknowledgement(detail), accept: licenseOk && downloadOk } });
      onInstalled();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setInstallId(null);
    }
  }

  const weightsMb = plan ? plan.plan.total_bytes / 1024 ** 2 : null;
  return (
    <article>
      <h3 className="model-title">{detail.id}</h3>
      <p className="model-hint">Revision <code>{detail.revision}</code> · <button className="link-button" onClick={() => void invoke("open_model_page", { repo: detail.id })}>Open on Hugging Face ↗</button></p>
      <StateRow inspection={inspection} loaded={library?.loaded ?? null} />
      <dl className="model-facts">
        <dt>Capability (source claim)</dt><dd>{detail.pipeline_tag ?? "not stated"}</dd>
        <dt>License</dt><dd>{licenseText(detail)}{detail.license_link && <> · <span className="model-hint">{detail.license_link}</span></>}{!detail.license && " — rights are not established"}</dd>
        <dt>Base model</dt><dd>{detail.base_models ?? "not stated"}</dd>
        <dt>Access</dt><dd>{detail.access === "public" ? "Public" : detail.access === "gated-accessible" ? "Gated · your token can read it" : "Gated · accept terms on Hugging Face and connect a token"}</dd>
        <dt>Content classification</dt><dd>{detail.audience === "not-for-all-audiences" ? "Flagged 'not for all audiences' by the source (18+)" : "Unverified (no reviewed classification)"}</dd>
        <dt>Format</dt><dd>{detail.format} · {detail.repo_files} files, {formatBytes(detail.repo_bytes)} in the repository</dd>
        {plan && <><dt>Download</dt><dd>{plan.plan.files.length} files · {formatBytes(plan.plan.total_bytes)}{plan.plan.variant ? ` (${plan.plan.variant} weights)` : ""} · disk free {formatBytes(free_bytes)}</dd></>}
        {plan && <><dt>Hardware</dt><dd>
          Weights need about {formatBytes(plan.plan.total_bytes)} of GPU memory before working space.
          {plan.verified?.measured_peak_vram_mb ? ` Measured peak on ${plan.verified.hardware_class}: ${(plan.verified.measured_peak_vram_mb / 1024).toFixed(1)} GB.` : " Peak usage not yet measured for this revision."}
          {" "}This PC: {gpu.name ?? "no NVIDIA GPU detected"}{gpu.vram_free_mb !== null && gpu.vram_total_mb !== null ? `, ${(gpu.vram_free_mb / 1024).toFixed(1)} of ${(gpu.vram_total_mb / 1024).toFixed(1)} GB free now` : ""}.
          {weightsMb !== null && gpu.vram_total_mb !== null && weightsMb + 1536 > gpu.vram_total_mb ? " This GPU is too small for these weights." : ""}
        </dd></>}
      </dl>
      <VideoMetadataCache inspection={inspection} onCached={onInstalled} />
      {detail.rejections.length > 0 && <div className="model-rejections"><strong>Why Optic Rift can't run this</strong><ul>{detail.rejections.map((r, i) => <li key={i}>{r.reason}</li>)}</ul></div>}
      {plan && !installed && (
        <div className="model-consent">
          <strong>Install {detail.id}</strong>
          <label><input type="checkbox" checked={licenseOk} onChange={(e) => setLicenseOk(e.target.checked)} /> I reviewed this model's license (<b>{licenseText(detail)}</b>) and will use its outputs accordingly. Optic Rift does not decide commercial rights.</label>
          <label><input type="checkbox" checked={downloadOk} onChange={(e) => setDownloadOk(e.target.checked)} /> Download {formatBytes(plan.plan.total_bytes)} from huggingface.co at revision {detail.revision.slice(0, 12)} into <code>{model_dir}</code>. Only safetensors weights and configuration files are downloaded; no repository code runs.</label>
          <div className="model-actions">
            <button className="primary" disabled={!licenseOk || !downloadOk || installId !== null || detail.access === "gated-needs-terms"} onClick={() => void install()}>Download and install</button>
            {installId && <button className="secondary" onClick={() => void invoke("model_install_cancel", { installId })}>Cancel</button>}
          </div>
        </div>
      )}
      {progress && installId && (
        <div className="model-progress" role="status">
          <progress max={progress.bytes_total || 1} value={progress.bytes_done} />
          <span>{progress.state} {progress.file ? `· ${progress.file} (${progress.file_index + 1}/${progress.file_count})` : ""} · {formatBytes(progress.bytes_done)} of {formatBytes(progress.bytes_total)}{progress.message ? ` · ${progress.message}` : ""}</span>
        </div>
      )}
      {error && <p className="project-error" role="alert">{error}</p>}
      {installed && <p className="model-hint">Installed {new Date(installed.manifest.installed_unix_ms).toLocaleString()} · {installed.manifest.files.length} verified files · license acknowledged: {installed.manifest.consent.license_acknowledged}</p>}
    </article>
  );
}

function VideoMetadataCache({ inspection, onCached }: { inspection: Inspection; onCached: () => void }) {
  const { detail, cached_metadata: cached } = inspection;
  const isVideo = ["text-to-video", "image-to-video", "video-to-video"].includes(detail.capability);
  const [licenseChecked, setLicenseChecked] = useState(false);
  const [transferChecked, setTransferChecked] = useState(false);
  const [adultChecked, setAdultChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");

  useEffect(() => {
    setLicenseChecked(false);
    setTransferChecked(false);
    setAdultChecked(false);
    setMessage("");
  }, [detail.id, detail.revision, detail.capability]);

  if (!isVideo) return null;

  const file = detail.cache_metadata_file;
  const flagged = detail.audience === "not-for-all-audiences";
  const ungated = detail.gated === "no";
  const canCache = Boolean(file && ungated && !cached);
  const ready = canCache && licenseChecked && transferChecked && (!flagged || adultChecked);

  async function cache() {
    if (!ready) return;
    setBusy(true);
    setMessage("");
    try {
      await invoke("video_cache_metadata", { request: {
        repo: detail.id,
        revision: detail.revision,
        capability: detail.capability,
        license_acknowledged: detail.license ?? "unknown",
        adult_acknowledged: flagged && adultChecked,
        accept: true,
      } });
      setMessage("Configuration metadata was verified and cached. No weights were downloaded.");
      onCached();
    } catch (e) {
      setMessage(errorText(e));
    } finally {
      setBusy(false);
    }
  }

  async function clear() {
    if (!cached || !window.confirm("Remove this model's cached configuration JSON? No weights or generated media will be touched.")) return;
    setBusy(true);
    setMessage("");
    try {
      await invoke("video_cache_clear", {
        repo: detail.id, revision: detail.revision, accept: true,
      });
      setMessage("Cached configuration cleared.");
      onCached();
    } catch (e) { setMessage(errorText(e)); }
    finally { setBusy(false); }
  }

  return <section className="model-consent" aria-label="Video configuration metadata cache">
    <strong>Video model metadata</strong>
    {cached ? <div>
        <p className="model-hint">Configuration metadata cached and verified on this computer. The model is not installed and cannot generate video yet.</p>
        <button type="button" className="secondary" disabled={busy} onClick={() => void clear()}>Clear cached configuration</button>
      </div>
      : !ungated ? <p className="model-hint">This model requires gated access. Optic Rift will not fetch its configuration through the public metadata-only cache.</p>
      : !file ? <p className="model-hint">No small configuration file with a verifiable source hash was found. This model remains discoverable but cannot be cached by this feature.</p>
      : <>
        <p className="model-hint">Optional: cache <code>{file.path}</code> ({formatBytes(file.size)}) from this exact revision. This transfers only JSON configuration metadata, not model weights or code. Video generation remains unsupported.</p>
        <label><input type="checkbox" checked={licenseChecked} onChange={(e) => setLicenseChecked(e.target.checked)} />
          I have reviewed the model license ({licenseText(detail)}), which may restrict use.</label>
        {flagged && <label><input type="checkbox" checked={adultChecked} onChange={(e) => setAdultChecked(e.target.checked)} />
          I am 18 or older and deliberately want to cache metadata for this source-flagged adult-focused model. This is not legal age verification.</label>}
        <label><input type="checkbox" checked={transferChecked} onChange={(e) => setTransferChecked(e.target.checked)} />
          I approve downloading this public configuration file from Hugging Face into Optic Rift's local metadata cache.</label>
        <button type="button" className="secondary" disabled={!ready || busy} onClick={() => void cache()}>
          {busy ? "Checking and caching…" : "Cache configuration metadata only"}
        </button>
      </>}
    {message && <p className="model-hint" role="status">{message}</p>}
  </section>;
}

function RuntimePanel({ library, onChanged }: { library: ModelLibrary | null; onChanged: () => void }) {
  const [consent, setConsent] = useState(false);
  const [busy, setBusy] = useState(false);
  const [log, setLog] = useState<string[]>([]);
  const [stage, setStage] = useState("");
  const [error, setError] = useState("");
  const runtime = library?.runtime;
  useEffect(() => {
    if (!busy) return;
    const stop = listen<{ stage: string; line: string | null }>("runtime-install", (e) => {
      setStage(e.payload.stage);
      if (e.payload.line) setLog((l) => [...l.slice(-6), e.payload.line as string]);
    });
    return () => { void stop.then((f) => f()); };
  }, [busy]);
  async function install() {
    setBusy(true); setError(""); setLog([]);
    try { await invoke("image_runtime_install", { uvSha256: runtime?.uv_executable?.sha256 ?? "" }); onChanged(); } catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  }
  if (!runtime) return null;
  return (
    <div className="runtime-panel">
      <div><strong>Image runtime</strong>
        {runtime.installed
          ? <small>Installed · Python {runtime.installed.python} · PyTorch {runtime.installed.torch} · diffusers {runtime.installed.diffusers} · {runtime.installed.device ?? "GPU"}</small>
          : <small>Needed to generate images. A private Python environment with pinned PyTorch ({runtime.torch}) and diffusion libraries, about 3.2 GB from download.pytorch.org and pypi.org, installed under <code>{runtime.root}</code>.{runtime.uv_available ? "" : " Requires uv (docs.astral.sh/uv), which was not found."}</small>}
      </div>
      {!runtime.installed && runtime.uv_executable && <p className="model-hint">Selected installer: <code>{runtime.uv_executable.path}</code> · SHA-256 <code>{runtime.uv_executable.sha256}</code>. Review where this executable came from before approving.</p>}
      {!runtime.installed && <div className="runtime-actions">
        <label><input type="checkbox" checked={consent} onChange={(e) => setConsent(e.target.checked)} /> I approve running the displayed uv executable and downloading these packages. The checksum identifies the file; it does not authenticate the publisher.</label>
        <button className="primary" disabled={!consent || busy || !runtime.uv_available || !runtime.uv_executable} onClick={() => void install()}>{busy ? "Installing…" : "Install runtime"}</button>
      </div>}
      {busy && <p className="model-hint" role="status">{stage}{log.length ? ` · ${log[log.length - 1]}` : ""}</p>}
      {error && <p className="project-error" role="alert">{error}</p>}
    </div>
  );
}

function InstalledPanel({ library, onChanged, onInspect }: { library: ModelLibrary | null; onChanged: () => void; onInspect: (repo: string, revision: string) => void }) {
  const [dir, setDir] = useState("");
  const [message, setMessage] = useState("");
  if (!library) return null;
  async function changeDir(event: FormEvent) {
    event.preventDefault();
    try { const p = await invoke<string>("models_set_dir", { path: dir }); setMessage(`Model folder set to ${p}. Models already installed elsewhere are not moved.`); setDir(""); onChanged(); }
    catch (e) { setMessage(errorText(e)); }
  }
  async function remove(repo: string, revision: string) {
    if (!window.confirm(`Remove ${repo} (${revision.slice(0, 12)}) from this computer?`)) return;
    try { await invoke("model_remove", { repo, revision }); onChanged(); } catch (e) { setMessage(errorText(e)); }
  }
  return (
    <div className="installed-panel">
      <strong>Installed models</strong>
      {library.installed.length === 0 && <p className="model-hint">None yet.</p>}
      <ul>
        {library.installed.map((m) => (
          <li key={m.dir}>
            <span><b>{m.manifest.repo}</b> <small>{m.manifest.revision.slice(0, 12)} · {formatBytes(m.manifest.total_bytes)} · {m.files_present ? "files verified at install" : "files missing"}{library.loaded && library.loaded[0] === m.manifest.repo ? " · loaded on GPU" : ""}</small></span>
            <span><button className="secondary" onClick={() => onInspect(m.manifest.repo, m.manifest.revision)}>Details</button> <button className="secondary" onClick={() => void remove(m.manifest.repo, m.manifest.revision)}>Remove</button></span>
          </li>
        ))}
      </ul>
      <form className="model-dir" onSubmit={changeDir}>
        <label className="field-label" htmlFor="model-dir">Model folder: <code>{library.model_dir}</code> ({formatBytes(library.free_bytes)} free)</label>
        <div><input id="model-dir" value={dir} onChange={(e) => setDir(e.target.value)} placeholder="New folder path, e.g. D:\Optic Rift Models" /><button className="secondary" type="submit" disabled={!dir.trim()}>Change folder</button></div>
      </form>
      {message && <p className="model-hint" role="status">{message}</p>}
    </div>
  );
}
