import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { saveWithPicturesConsent } from "./exportPrivacy";
import type { Provenance } from "./models";

type MediaResult = { name: string; url: string; size: number };
type Props = { refreshVersion: number; onResult: (media: MediaResult) => void };

const readableError = (error: unknown) =>
  typeof error === "string" ? error : error instanceof Error ? error.message : "The desktop bridge is unavailable.";

/** Existing app-owned local outputs survive restarts; they are NOT project assets. */
export default function GenerationHistory({ refreshVersion, onResult }: Props) {
  const [items, setItems] = useState<Provenance[]>([]);
  const [refresh, setRefresh] = useState(0);
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError("");
    void invoke<Provenance[]>("generation_outputs")
      .then((records) => {
        if (active) setItems(records.slice(0, 20));
      })
      .catch((e) => { if (active) setError(readableError(e)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [refreshVersion, refresh]);

  async function open(item: Provenance) {
    setWorking(item.job_id);
    setError("");
    setMessage("");
    let objectUrl: string | null = null;
    try {
      const dataUri = await invoke<string>("generation_output_image", { jobId: item.job_id });
      const response = await fetch(dataUri);
      if (!response.ok) throw new Error("The stored image could not be opened.");
      const blob = await response.blob();
      if (blob.type !== "image/png" || blob.size === 0) {
        throw new Error("The stored output is not a valid PNG response.");
      }
      objectUrl = URL.createObjectURL(blob);
      onResult({
        name: `generated-${item.job_id.slice(0, 8)}.png`,
        url: objectUrl,
        size: blob.size,
      });
    } catch (e) {
      if (objectUrl) URL.revokeObjectURL(objectUrl);
      setError(readableError(e));
    } finally {
      setWorking(null);
    }
  }

  async function save(item: Provenance) {
    setWorking(item.job_id);
    setError("");
    setMessage("");
    try {
      const path = await saveWithPicturesConsent(
        item.job_id,
        (warning) => window.confirm(warning),
        (jobId) => invoke<string>("generation_save", { jobId }),
      );
      if (path !== null) setMessage(`Saved copy to ${path}`);
    } catch (e) {
      setError(readableError(e));
    } finally {
      setWorking(null);
    }
  }

  return <section className="generation-history" aria-label="Recent local generations">
    <div className="history-heading">
      <div><h3>Recent generations</h3><p>Images saved locally by Optic Rift, including previous sessions.</p></div>
      <button className="secondary" type="button" onClick={() => setRefresh(v => v + 1)} disabled={loading || working !== null}>Refresh</button>
    </div>
    {loading && <p className="model-hint" role="status">Loading local generations…</p>}
    {!loading && items.length === 0 && !error && <p className="model-hint">No generated images saved locally yet.</p>}
    <ol className="history-items">
      {items.map((item) => <li key={item.job_id}>
        <div className="history-summary">
          <strong title={item.prompt}>{item.prompt || "Untitled generation"}</strong>
          <small>{item.model.repo} · {item.output.width}×{item.output.height} · {new Date(item.created_unix_ms).toLocaleString()}</small>
        </div>
        <div className="model-actions">
          <button className="secondary" type="button" disabled={working !== null} onClick={() => void open(item)}>{working === item.job_id ? "Working…" : "Open"}</button>
          <button className="secondary" type="button" disabled={working !== null} onClick={() => void save(item)}>Save copy</button>
        </div>
      </li>)}
    </ol>
    {error && <p className="project-error" role="alert">{error}</p>}
    {message && <p className="model-hint" role="status">{message}</p>}
    <p className="model-hint">These files remain in the local output folder. Opening one in the Studio is session-only; permanent project asset linking is not available yet.</p>
  </section>;
}
