/** Defensive display-only view of app-local history. This is NOT the native trust gate. */
export type LocalImageSource = {
  id: string;
  label: string;
  width: number;
  height: number;
};

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const SHA256 = /^[0-9a-f]{64}$/;

export function localImageSourceChoices(records: unknown): LocalImageSource[] {
  if (!Array.isArray(records)) return [];
  const choices: LocalImageSource[] = [];
  for (const entry of records) {
    if (!entry || typeof entry !== "object") continue;
    const p = entry as Record<string, unknown>;
    const output = p.output;
    if (!output || typeof output !== "object") continue;
    const image = output as Record<string, unknown>;
    const width = image.width;
    const height = image.height;
    if (typeof p.job_id !== "string" || !UUID.test(p.job_id)
      || typeof image.sha256 !== "string" || !SHA256.test(image.sha256)
      || typeof width !== "number" || typeof height !== "number"
      || !Number.isInteger(width) || !Number.isInteger(height)
      || width < 256 || width > 1536 || height < 256 || height > 1536
      || width % 8 !== 0 || height % 8 !== 0) continue;
    const prompt = typeof p.prompt === "string" ? p.prompt.trim().slice(0, 36) : "";
    choices.push({ id: p.job_id, label: prompt || "Untitled", width, height });
    if (choices.length === 20) break;
  }
  return choices;
}
