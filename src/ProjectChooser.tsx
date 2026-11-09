import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  formatCreated,
  MAX_PROJECT_NAME_CHARS,
  projectClient,
  projectNameError,
  type ProjectListing,
  type ProjectRecord
} from "./projects";

type Props = { current: ProjectRecord | null; onOpened: (project: ProjectRecord) => void };

export default function ProjectChooser({ current, onOpened }: Props) {
  const client = useMemo(() => projectClient(invoke), []);
  const [listing, setListing] = useState<ProjectListing | null>(null);
  const [loadError, setLoadError] = useState("");
  const [name, setName] = useState("");
  const [formError, setFormError] = useState("");
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    const result = await client.list();
    if (result.ok) {
      setListing(result.value);
      setLoadError("");
    } else {
      setListing(null);
      setLoadError(result.error);
    }
  }, [client]);

  useEffect(() => { void refresh(); }, [refresh]);

  async function create(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    const result = await client.create(name);
    setBusy(false);
    if (!result.ok) { setFormError(result.error); return; }
    setName("");
    setFormError("");
    onOpened(result.value);
    await refresh();
  }

  async function open(id: string) {
    setBusy(true);
    const result = await client.open(id);
    setBusy(false);
    if (result.ok) onOpened(result.value);
    else setLoadError(result.error);
  }

  const liveError = name.length > 0 ? projectNameError(name) : null;

  return (
    <main className="content-page projects-page">
      <div className="page-eyebrow">PROJECTS</div>
      <h1>Choose a project.</h1>
      <p className="lead">Projects are saved on this computer and are available after you restart Optic Rift. Projects store their names and explicitly saved PNG originals with versioned photo edits. Imported media is still session-only until saved; video and JPEG/WebP project storage are not yet supported.</p>

      <form className="project-create" onSubmit={create}>
        <label className="field-label" htmlFor="project-name">New project name</label>
        <div className="project-create-row">
          <input id="project-name" value={name} maxLength={MAX_PROJECT_NAME_CHARS * 2} onChange={(event) => { setName(event.target.value); setFormError(""); }} placeholder="e.g. Coastline short film" autoComplete="off" aria-invalid={Boolean(liveError || formError)} aria-describedby="project-name-error" />
          <button className="primary" type="submit" disabled={busy || Boolean(liveError) || name.trim().length === 0}>Create project</button>
        </div>
        <p id="project-name-error" className="project-error" role="alert">{formError || liveError || ""}</p>
      </form>

      <div className="project-list-heading"><strong>Saved projects</strong><button className="secondary" onClick={() => void refresh()} disabled={busy}>Refresh</button></div>
      {loadError && <p className="project-error" role="alert">{loadError}</p>}
      {listing && listing.projects.length === 0 && <p className="disclosure">No saved projects yet.</p>}
      {listing && listing.projects.length > 0 && <ul className="project-list">
        {listing.projects.map((project) => <li key={project.id} className={current?.id === project.id ? "project-row active" : "project-row"}>
          <span><strong>{project.name}</strong><small>Created {formatCreated(project.created_unix_ms)}</small></span>
          <button className="secondary" onClick={() => void open(project.id)} disabled={busy}>{current?.id === project.id ? "Open (current)" : "Open"}</button>
        </li>)}
      </ul>}
      {listing && listing.problems.length > 0 && <div className="project-problems" role="status">
        <strong>{listing.problems.length} project record{listing.problems.length === 1 ? "" : "s"} could not be read</strong>
        <ul>{listing.problems.map((problem) => <li key={problem.file_name}><code>{problem.file_name}</code>: {problem.error}</li>)}</ul>
        <p>These files were left untouched.</p>
      </div>}
    </main>
  );
}
