/** Read-only, intentionally untrusted public generation disclosure.
 * C2PA verification must be a separate native trust decision; an unsigned
 * local receipt never becomes a certificate by virtue of passing this parser.
 */
export type PublicModelStage = {
  jobId: string;
  outputSha256: string;
  modelRepo: string | null;
  modelRevision: string | null;
  adapter: string | null;
  adapterVersion: string | null;
};
export type PublicProvenanceView = {
  status: "unsigned_unverified_not_c2pa";
  involvement: "ai_generated";
  assertionOrigin: "optic_rift_local_unsigned_record";
  lineage: "complete_within_available_local_history" | "incomplete";
  outputSha256: string;
  stages: PublicModelStage[];
};
const object = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);
const digest = (value: unknown): value is string =>
  typeof value === "string" && /^[0-9a-f]{64}$/i.test(value);
const jobId = (value: unknown): value is string =>
  typeof value === "string" &&
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(value);
const safeSegment = (value: string): boolean =>
  value.length > 0 && !value.startsWith(".") && !value.endsWith(".") &&
  !value.includes("..") && /^[a-zA-Z0-9._-]+$/.test(value);
function optionalModelId(value: unknown, field: "repo" | "revision" | "adapter" | "adapter_version"): string | null {
  if (value === null) return null;
  if (typeof value !== "string") throw new Error("The model disclosure contains an invalid identifier.");
  const max = field === "repo" ? 160 : field === "revision" || field === "adapter" ? 120 : 80;
  if (value.length > max) throw new Error("The model disclosure identifier is too long.");
  if (field === "repo") {
    const parts = value.split("/");
    if (parts.length !== 2 || !parts.every(safeSegment)) {
      throw new Error("The model disclosure contains a path-shaped repository.");
    }
  } else if (!safeSegment(value)) {
    throw new Error("The model disclosure contains a path-shaped identifier.");
  }
  return value;
}

/** Accept only the expected public fields. Never retain/reveal unexpected
 * properties such as prompts or native file paths.
 */
export function parsePublicProvenance(value: unknown): PublicProvenanceView {
  if (!object(value) || value.record_version !== 2 ||
      value.kind !== "optic-rift-ai-generation-disclosure" ||
      value.ai_involvement !== "ai_generated" ||
      value.assertion_origin !== "optic_rift_local_unsigned_record" ||
      value.credential_status !== "unsigned_unverified_not_c2pa" ||
      (value.lineage_status !== "complete_within_available_local_history" &&
       value.lineage_status !== "incomplete") ||
      value.stage_order !== "earliest_to_latest" ||
      !object(value.output) || !digest(value.output.sha256) ||
      !Array.isArray(value.model_stages) ||
      value.model_stages.length < 1 || value.model_stages.length > 16) {
    throw new Error("The local AI disclosure is missing required unverified-provenance fields.");
  }
  const stages: PublicModelStage[] = value.model_stages.map((stage: unknown) => {
    if (!object(stage) || !jobId(stage.job_id) || !digest(stage.output_sha256)) {
      throw new Error("The local disclosure has an invalid model stage.");
    }
    return {
      jobId: stage.job_id, outputSha256: stage.output_sha256,
      modelRepo: optionalModelId(stage.model_repo, "repo"),
      modelRevision: optionalModelId(stage.model_revision, "revision"),
      adapter: optionalModelId(stage.adapter, "adapter"),
      adapterVersion: optionalModelId(stage.adapter_version, "adapter_version"),
    };
  });
  if (stages.at(-1)?.outputSha256 !== value.output.sha256) {
    throw new Error("The final model stage does not match the disclosed output digest.");
  }
  return {
    status: "unsigned_unverified_not_c2pa",
    involvement: "ai_generated",
    assertionOrigin: "optic_rift_local_unsigned_record",
    lineage: value.lineage_status,
    outputSha256: value.output.sha256,
    stages,
  };
}
