import { useState } from "react";
import GenerationHistory from "./GenerationHistory";
import { editorFor, type MediaItem } from "./creativeNavigation";
import { formatSize } from "./mediaFormat";
import type { ProjectPhotoDocument, ProjectPhotoProblem } from "./projectPhotos";

type KindFilter = "all" | "image" | "video";
type Props = {
  items: readonly MediaItem[];
  projectName: string | null;
  notification: string;
  onDismissNotification: () => void;
  historyVersion: number;
  onImport: () => void;
  onOpen: (item: MediaItem) => void;
  onRemove: (id: string) => void;
  onGeneratedResult: (result: { name: string; url: string; size: number }) => void;
  savedPhotos: readonly ProjectPhotoDocument[];
  savedPhotoProblems: readonly ProjectPhotoProblem[];
  possibleIncompleteSaves: number;
  savedPhotoBusy: boolean;
  savedPhotoNotice: string;
  onOpenSavedPhoto: (assetId: string) => void;
};

/** Privacy-conscious session media: no unsolicited media thumbnails, no
 * pretend project persistence or unqualified timeline/AI operations.
 */
export default function SessionMediaLibrary({
  items, projectName, notification, onDismissNotification, historyVersion, onImport, onOpen, onRemove, onGeneratedResult,
  savedPhotos, savedPhotoProblems, possibleIncompleteSaves, savedPhotoBusy, savedPhotoNotice, onOpenSavedPhoto,
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
      Opened media is session-only unless you explicitly save a PNG into the active project.
      Managed project PNG copies and edit recipes can be reopened locally after restarting.
    </p>
    {notification && <div className="session-library-notice" role="status">
      <span>{notification}</span><button type="button" onClick={onDismissNotification}
        aria-label="Dismiss media notification">×</button>
    </div>}
    {projectName && <p className="session-project-note">Current project: <strong>{projectName}</strong>. Session media below is not automatically saved.</p>}
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
          {item.derivedFrom?.kind === "video-frame" && <small className="session-media-origin">
            Frame captured at {item.derivedFrom.timeSeconds.toFixed(3)} s from {item.derivedFrom.sourceName}. Not saved as project provenance.
          </small>}
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
    {projectName && <section className="session-saved-photos" aria-label="Saved project PNG photos">
      <h2>Saved PNG photos in this project</h2>
      <p>Reopen locally managed PNG originals and their latest saved editing settings.
        Other media remains session-only. These project files are not a portable bundle.</p>
      {savedPhotoNotice && <p role="status">{savedPhotoNotice}</p>}
      {possibleIncompleteSaves > 0 && <div className="project-problems" role="status" aria-label="Possible incomplete photo saves">
        <strong>{possibleIncompleteSaves} possible unfinished project photo save(s)</strong>
        <p>These are staging-shaped entries, not confirmed saved photos. Nothing was deleted or restored.
          Incomplete files may use disk space. Automatic cleanup is not enabled while recovery policy is unresolved.</p>
      </div>}
      {!savedPhotos.length && !savedPhotoProblems.length &&
        <p className="session-library-empty">No PNG photos explicitly saved to this project yet.</p>}
      <ul className="session-media-grid">
        {savedPhotos.map((photo) => <li className="session-media-card" key={photo.asset_id}>
          <div className="session-media-symbol" aria-hidden="true">▧</div>
          <div className="session-media-detail">
            <strong title={photo.source_name}>{photo.source_name}</strong>
            <small>Managed PNG · Revision {photo.revision} · {photo.source_width} × {photo.source_height} source pixels</small>
          </div>
          <div className="session-media-actions">
            <button type="button" className="secondary" disabled={savedPhotoBusy}
              onClick={() => onOpenSavedPhoto(photo.asset_id)}>Reopen editable photo</button>
          </div>
        </li>)}
      </ul>
      {savedPhotoProblems.length > 0 && <div className="project-problems" role="status">
        <strong>{savedPhotoProblems.length} saved photo record(s) could not be read</strong>
        <ul>{savedPhotoProblems.map((problem) =>
          <li key={problem.asset_id}><code>{problem.asset_id}</code>: {problem.error}</li>)}</ul>
        <p>Unreadable records were not silently deleted or modified.</p>
      </div>}
    </section>}
    <section className="session-generated-section" aria-label="Local generated outputs">
      <h2>Recent generated images</h2>
      <p>Verified generated outputs can remain in Optic Rift's local output folder after restarting.
        Opening one adds it to this session, not to a saved project. Previews remain hidden until you choose to open them.</p>
      <GenerationHistory refreshVersion={historyVersion} onResult={onGeneratedResult} />
    </section>
  </main>;
}
