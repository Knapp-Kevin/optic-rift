// Client types and pure logic for model discovery, installation and local generation.
// The native layer is the authority; this module only shapes data for display.

export type Capability =
  | "text-to-image" | "image-to-image" | "inpainting" | "upscaling" | "restoration"
  | "image-to-video" | "text-to-video" | "video-to-video";

export type CapabilityInfo = { id: Capability; label: string; adapters: string[] };
export type Compatibility =
  | { kind: "no-adapter" }
  | { kind: "unsupported-format"; detail: string }
  | { kind: "inspect-to-confirm" };

export type ModelSummary = {
  id: string; sha: string | null; pipeline_tag: string | null; library: string | null;
  downloads: number; likes: number; gated: string; license: string | null; last_modified: string | null;
  audience: "not-for-all-audiences" | "unverified"; format: string; compatibility: Compatibility;
};
export type SearchResult = { capability: Capability; models: ModelSummary[]; hidden_not_for_all_audiences: number };

export type RepoFile = { path: string; size: number; lfs_sha256: string | null; blob_id: string | null };
export type InstallPlan = { adapter: string; files: RepoFile[]; total_bytes: number; variant: string | null };
export type VerifiedReceipt = { repo: string; revision: string; adapter: string; hardware_class: string; evidence: string; measured_peak_vram_mb: number | null };
export type PlanView = { plan: InstallPlan; adapter_label: string; verified: VerifiedReceipt | null };
export type Rejection = { adapter: string; reason: string };
export type Access = "public" | "gated-accessible" | "gated-needs-terms";
export type ModelDetail = {
  id: string; revision: string; pipeline_tag: string | null; capability: Capability;
  license: string | null; license_name: string | null; license_link: string | null; base_models: string | null;
  gated: string; access: Access; audience: string; format: string; repo_files: number; repo_bytes: number;
  cache_metadata_file: RepoFile | null;
  plans: PlanView[]; rejections: Rejection[]; page_url: string;
};
export type GpuInfo = { name: string | null; driver: string | null; vram_total_mb: number | null; vram_used_mb: number | null; vram_free_mb: number | null };
export type Manifest = {
  repo: string; revision: string; adapter: string; adapter_version: number; variant: string | null;
  license: string | null; total_bytes: number; installed_unix_ms: number;
  consent: { license_acknowledged: string; acknowledged_unix_ms: number };
  files: { path: string; size: number; sha256: string }[];
};
export type InstalledModel = { manifest: Manifest; dir: string; files_present: boolean };
export type Inspection = { detail: ModelDetail; cached_metadata: boolean; installed: InstalledModel | null; gpu: GpuInfo; runtime_installed: boolean; model_dir: string; free_bytes: number | null };
export type RuntimeManifest = { runtime: string; python: string; torch: string; diffusers: string; cuda_available: boolean; device: string | null };
export type RuntimeStatus = { installed: RuntimeManifest | null; uv_available: boolean; uv_executable: { path: string; sha256: string } | null; root: string; torch_index: string; torch: string; packages: string[] };
export type ModelLibrary = { model_dir: string; free_bytes: number | null; installed: InstalledModel[]; loaded: [string, string] | null; runtime: RuntimeStatus; gpu: GpuInfo };
export type InstallEvent = { install_id: string; repo: string; state: string; file: string | null; file_index: number; file_count: number; bytes_done: number; bytes_total: number; message: string | null };
export type Provenance = {
  job_id: string; created_unix_ms: number; prompt: string; negative_prompt: string;
  parameters: { width: number; height: number; steps: number; guidance: number; seed: number };
  model: { repo: string; revision: string; adapter: string; license: string | null };
  runtime: { torch: string; diffusers: string; device: string | null };
  output: { sha256: string; width: number; height: number; bytes: number };
  metrics: { generation_seconds: number | null; load_seconds: number | null; peak_vram_allocated_mb: number | null };
  project_id: string | null; lineage_note: string;
};

export type StateFlag = { reached: boolean; detail: string };
export type ModelStates = { discovered: StateFlag; supported: StateFlag; verified: StateFlag; installed: StateFlag; ready: StateFlag };

/** Five independent states. Each is earned by evidence, never implied by another. */
export function modelStates(inspection: Inspection, loaded: [string, string] | null = null): ModelStates {
  const { detail, installed, runtime_installed, gpu } = inspection;
  const plan = detail.plans[0] ?? null;
  const supported: StateFlag = plan
    ? { reached: true, detail: `Runs with ${plan.adapter_label}` }
    : { reached: false, detail: detail.rejections[0]?.reason ?? "We found this model, but Optic Rift doesn't yet know how to run it." };
  const verified: StateFlag = plan?.verified
    ? { reached: true, detail: `Qualified on ${plan.verified.hardware_class}` }
    : { reached: false, detail: plan ? "Not yet qualified on this revision and hardware class." : "Not applicable until supported." };
  const isInstalled = Boolean(installed && installed.files_present);
  const installedFlag: StateFlag = isInstalled
    ? { reached: true, detail: "Verified files are installed locally." }
    : { reached: false, detail: installed ? "Installed files are missing or incomplete." : "Not installed." };
  let readyDetail = "Ready to generate.";
  if (!isInstalled) readyDetail = "Install the model first.";
  else if (!runtime_installed) readyDetail = "Install the image runtime first.";
  else if (gpu.vram_total_mb === null) readyDetail = "No supported NVIDIA GPU was detected.";
  const ready = isInstalled && runtime_installed && gpu.vram_total_mb !== null;
  const loadedNow = Boolean(loaded && installed && loaded[0] === installed.manifest.repo && loaded[1] === installed.manifest.revision);
  return {
    discovered: { reached: true, detail: `Listed on Hugging Face at ${detail.revision.slice(0, 12)}` },
    supported,
    verified,
    installed: installedFlag,
    ready: { reached: ready, detail: ready && loadedNow ? "Loaded on the GPU." : readyDetail },
  };
}

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes)) return "unknown";
  if (bytes < 1024 ** 2) return (bytes / 1024).toFixed(1) + " KB";
  if (bytes < 1024 ** 3) return (bytes / 1024 ** 2).toFixed(1) + " MB";
  return (bytes / 1024 ** 3).toFixed(2) + " GB";
}

export function licenseText(detail: Pick<ModelDetail, "license" | "license_name">): string {
  if (!detail.license) return "unknown";
  return detail.license === "other" && detail.license_name ? `other (${detail.license_name})` : detail.license;
}

/** Value the user must acknowledge; must match what the native layer re-reads. */
export function licenseAcknowledgement(detail: Pick<ModelDetail, "license">): string {
  return detail.license ?? "unknown";
}

export function compatibilityText(c: Compatibility): string {
  switch (c.kind) {
    case "no-adapter": return "No Optic Rift adapter for this capability yet";
    case "unsupported-format": return c.detail;
    case "inspect-to-confirm": return "Diffusers format: inspect to check compatibility";
  }
}

export type GenerationSettings = { width: number; height: number; steps: number; guidance: number; seed: number | null };
export const SIZE_PRESETS: { label: string; width: number; height: number }[] = [
  { label: "Square 1024 × 1024", width: 1024, height: 1024 },
  { label: "Landscape 1216 × 832", width: 1216, height: 832 },
  { label: "Portrait 832 × 1216", width: 832, height: 1216 },
  { label: "Small 768 × 768", width: 768, height: 768 },
];

/** Mirrors the worker's limits so obvious mistakes are caught before a GPU round trip. */
export function settingsError(prompt: string, s: GenerationSettings): string | null {
  if (!prompt.trim()) return "Describe what you want to create.";
  if (prompt.length > 2000) return "Keep the prompt under 2,000 characters.";
  for (const [k, v] of [["width", s.width], ["height", s.height]] as const) {
    if (!Number.isInteger(v) || v < 256 || v > 1536 || v % 8) return `${k} must be 256–1536 and a multiple of 8.`;
  }
  if (!Number.isInteger(s.steps) || s.steps < 1 || s.steps > 100) return "Steps must be 1–100.";
  if (!Number.isFinite(s.guidance) || s.guidance < 0 || s.guidance > 20) return "Guidance must be 0–20.";
  if (s.seed !== null && (!Number.isInteger(s.seed) || s.seed < 0 || s.seed >= 2 ** 32)) return "Seed must be a whole number from 0 to 4294967295.";
  return null;
}

export type GpuProcess = { pid: number; name: string; dedicated_mb: number; engine_3d_percent: number; own: boolean };
export type GpuSnapshot = { device: string | null; total_mb: number | null; used_mb: number | null; free_mb: number | null; processes: GpuProcess[]; per_process: boolean };
export type GpuDecision =
  | { kind: "proceed"; budget_mb: number }
  | { kind: "confirm"; budget_mb: number; apps: GpuProcess[]; message: string }
  | { kind: "block"; message: string };
export type GpuCheck = { snapshot: GpuSnapshot; decision: GpuDecision; need_mb: number; idle_unload_seconds: number };
