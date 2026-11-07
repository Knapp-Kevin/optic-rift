import { useState } from "react";
import GenerationHistory from "./GenerationHistory";
import { editorFor, type MediaItem } from "./creativeNavigation";
import { formatSize } from "./mediaFormat";

type KindFilter = "all" | "image" | "video";
type Props = {
  items: readonly MediaItem[];
  projectName: string | null;
  historyVersion: number;
  onImport: () => void;
  onOpen: (item: MediaItem) => void;
  onRemove: (id: string) => void;
  onGeneratedResult: (result: { name: string; url: string; size: number }) => void;
};

/** Privacy-conscious session media: no unsolicited media thumbnails, no
 * pretend project persistence or unqualified timeline/AI operations.
 */
export default function SessionMediaLibrary({
  items, projectName, historyVersion, onImport, onOpen, onRemove, onGeneratedResult,
}: Props) {
  const [filter, setFilter] = useState<KindFilter>("all");
  const [query, setQuery] = useState("");
  const lowered = query.trim().toLocaleLowerCase();
  const visible = items.filter((item) =>
    (filter === "all" || item.kind === filter) &&
    (lowered === "" || item.name.toLocaleLowerCase().includes(lowered))
  );
  const photos = items.filter((item) => item.kind === "image").length;
  const clips = items.length - photos;

  return <main className="content-page session-library-page">
    <div className="page-eyebrow">MEDIA LIBRARY</div>
    <h1>Your visual workspace.</h1>
    <p className="lead">
      Find the photos and videos you've opened, then continue in the right Studio.
      These opened files are available only during this session. A project does not yet
      store imported media.
    </p>
    {projectName && <p className="session-project-note">Current project: <strong>{projectName}</strong>. Media below is not yet attached to this project.</p>}
    <div className="session-media-toolbar">
      <button type="button" className="primary" onClick={onImport}>＋ Import photos or videos</button>
      <label className="session-search">Search opened media
        <input type="search" value={query} onChange={(e) => setQuery(e.target.value)}
          placeholder="Filter by filename" />
      </label>
    </div>
    <div className="session-media-filter" role="group" aria-label="Filter opened media">
      <button type="button" aria-pressed={filter === "all"} className={filter === "all" ? "selected" : ""}
        onClick={() => setFilter("all")}>All ({items.length})</button>
      <button type="button" aria-pressed={filter === "image"} className={filter === "image" ? "selected" : ""}
        onClick={() => setFilter("image")}>Photos ({photos})</button>
      <button type="button" aria-pressed={filter === "video"} className={filter === "video" ? "selected" : ""}
        onClick={() => setFilter("video")}>Videos ({clips})</button>
    </div>
    {visible.length === 0 && <div className="session-library-empty">
      {items.length === 0 ? "No files opened yet. Import a photo or video to begin." : "No opened media matches this filter."}
    </div>}
    <ul className="session-media-grid">
      {visible.map((item) => <li className="session-media-card" key={item.id}>
        <div className="session-media-symbol" aria-hidden="true">{item.kind === "image" ? "▧" : "▶"}</div>
        <div className="session-media-detail">
          <strong title={item.name}>{item.name}</strong>
          <small>{item.kind === "image" ? "Photo" : "Video"} · {formatSize(item.size)} · Session only</small>
        </div>
        <div className="session-media-actions">
          <button type="button" className="secondary" onClick={() => onOpen(item)}>
            {editorFor(item.kind) === "photo" ? "Open Photo Studio" : "Open Video Studio"}
          </button>
          <button type="button" className="session-remove" aria-label={"Remove " + item.name + " from this session"}
            title="Remove from this session, not from disk" onClick={() => onRemove(item.id)}>Remove</button>
        </div>
      </li>)}
    </ul>
    <section className="session-generated-section" aria-label="Local generated outputs">
      <h2>Recent generated images</h2>
      <p>Verified generated outputs can remain in Optic Rift's local output folder after restarting.
        Opening one adds it to this session, not to a saved project. Previews remain hidden until you choose to open them.</p>
      <GenerationHistory refreshVersion={historyVersion} onResult={onGeneratedResult} />
    </section>
  </main>;
}
