import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import GenerationThumbnail from "./GenerationThumbnail";
import type { Provenance } from "./models";

const message = (e: unknown) => e instanceof Error ? e.message : typeof e === "string" ? e : "Could not open this image.";

type Props = {
  refreshVersion: number;
  selectedId: string | null;
  onSelect: (item: Provenance, imageUrl: string) => void;
  onError: (message: string) => void;
  /** False while Create is hidden: revealed previews are concealed again and their data dropped. */
  active?: boolean;
};

export default function GenerationFilmstrip({ refreshVersion, selectedId, onSelect, onError, active = true }: Props) {
  const [records, setRecords] = useState<Provenance[]>([]);
  const [loading, setLoading] = useState(true);
  const [working, setWorking] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const [previewsRevealed, setPreviewsRevealed] = useState(false);
  useEffect(() => { if (!active) setPreviewsRevealed(false); }, [active]);

  useEffect(() => {
    let active = true;
    setLoading(true);
    void invoke<Provenance[]>("generation_outputs")
      .then((result) => { if (active) setRecords(result.slice(0, 20)); })
      .catch((e) => { if (active) { setRecords([]); onError(message(e)); } })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [refreshVersion, refresh, onError]);

  async function choose(item: Provenance) {
    setWorking(item.job_id);
    try {
      // Full-sized image is fetched only after the user selects a thumbnail.
      const uri = await invoke<string>("generation_output_image", { jobId: item.job_id });
      if (!uri.startsWith("data:image/png;base64,")) throw new Error("Native preview returned an unexpected image.");
      onSelect(item, uri);
    } catch (e) {
      onError(message(e));
    } finally {
      setWorking(null);
    }
  }

  return <section className="create-filmstrip" aria-label="Recent local generations">
    <div className="create-filmstrip-heading">
      <div><strong>Recent generations</strong><small>Choose an image to view or transform. Local files survive restarts.</small></div>
      <div className="model-actions">
        <button type="button" className="secondary" aria-pressed={previewsRevealed} onClick={() => setPreviewsRevealed((shown) => !shown)}>
          {previewsRevealed ? "Hide previews" : "Show previews"}
        </button>
        <button type="button" className="secondary" disabled={working !== null} onClick={() => setRefresh(n => n + 1)}>Refresh</button>
      </div>
    </div>
    {!previewsRevealed && <small className="model-hint">Previews stay hidden until you choose to show them. Local images may contain sensitive content.</small>}
    <div className="create-thumbnails" role="list">
      {records.map((item) => <div role="listitem" key={item.job_id}>
        <button type="button" className={selectedId === item.job_id ? "create-thumbnail selected" : "create-thumbnail"}
          aria-label={`Select image generated on ${new Date(item.created_unix_ms).toLocaleString()}`}
          aria-pressed={selectedId === item.job_id} title={new Date(item.created_unix_ms).toLocaleString()}
          disabled={working !== null} onClick={() => void choose(item)}>
          {previewsRevealed ? <GenerationThumbnail jobId={item.job_id} /> : <span className="generation-thumbnail" aria-hidden="true">▧</span>}
          <span className="create-thumbnail-name">{item.source ? "Variation" : "Image"} · {item.output.width}×{item.output.height}</span>
        </button>
      </div>)}
      {loading && <span className="model-hint" role="status">Loading history…</span>}
      {!loading && records.length === 0 && <span className="model-hint">Your generated images will appear here.</span>}
    </div>
  </section>;
}
