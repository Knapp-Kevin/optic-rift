import { useEffect, useRef, useState, type ChangeEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import ProjectChooser from "./ProjectChooser";
import HuggingFaceAccess from "./HuggingFaceAccess";
import ModelBrowser from "./ModelBrowser";
import { DownloadProvider, DownloadStatus } from "./downloads";
import GeneratePanel from "./GeneratePanel";
import GenerationHistory from "./GenerationHistory";
import type { ProjectRecord } from "./projects";
import { projectPhotosClient, editsFromRecipe, restoredPngBlob, type ProjectPhotoDocument, type ProjectPhotoProblem } from "./projectPhotos";
import { navigation, routeTitle, editorFor, mediaForEditor, type View, type MediaItem } from "./creativeNavigation";
import { formatSize, IMAGE_PREVIEW_FAILED, VIDEO_PREVIEW_FAILED } from "./mediaFormat";
import {
  EMPTY_PHOTO_CROP, centeredPhotoCrop, photoResizedOutputDimensions, renderAdjustedPhoto, validatePhotoCrop,
  type PhotoAdjustments, type PhotoCrop,
} from "./photoEdits";
import { applyPhotoEdit, createPhotoHistory, finishPhotoGesture, undoPhotoEdit, redoPhotoEdit, DEFAULT_PHOTO_EDITS, type PhotoEditHistory } from "./photoHistory";
import { captureVideoFrame, capturedFrameName } from "./videoFrameCapture";
import PhotoCanvasPreview from "./PhotoCanvasPreview";
import SessionMediaLibrary from "./SessionMediaLibrary";
import VideoShotComposer from "./VideoShotComposer";
import VideoEnhancePlanner from "./VideoEnhancePlanner";
import CreativeWorkflowExplorer from "./CreativeWorkflowExplorer";
import type { Capability } from "./models";
import type { VideoShotMode } from "./videoShotPlan";

type RuntimeStatus = { application: string; stage: string; generation_ready: boolean; project_storage_ready: boolean };

// Seeking moves in 0.1 s steps, so labels show tenths; rounding first absorbs float drift (20 x 0.1 = 1.9999...).
const formatTime = (seconds: number) => {
  const tenths = Math.round(Math.max(0, Number.isFinite(seconds) ? seconds : 0) * 10);
  const m = Math.floor(tenths / 600);
  const s = (tenths % 600) / 10;
  return String(m).padStart(2, "0") + ":" + s.toFixed(1).padStart(4, "0");
};

export default function App() {
  const [view, setView] = useState<View>("projects");
  const [project, setProject] = useState<ProjectRecord | null>(null);
  const [items, setItems] = useState<MediaItem[]>([]);
  const [selectedPhotoId, setSelectedPhotoId] = useState<string | null>(null);
  const [selectedVideoId, setSelectedVideoId] = useState<string | null>(null);
  const [photoHistories, setPhotoHistories] = useState<Record<string, PhotoEditHistory>>({});
  const [photoDimensions, setPhotoDimensions] = useState<Record<string, { width: number; height: number }>>({});
  const [savedProjectPhotos, setSavedProjectPhotos] = useState<ProjectPhotoDocument[]>([]);
  const [savedPhotoProblems, setSavedPhotoProblems] = useState<ProjectPhotoProblem[]>([]);
  const [projectPhotoBusy, setProjectPhotoBusy] = useState(false);
  const [projectPhotoNotice, setProjectPhotoNotice] = useState("");
  const [prompt, setPrompt] = useState("");
  const [createOpened, setCreateOpened] = useState(false);
  const [videoCreateOpened, setVideoCreateOpened] = useState(false);
  const [videoEnhanceOpened, setVideoEnhanceOpened] = useState(false);
  const [modelBrowseCapability, setModelBrowseCapability] = useState<Capability>("text-to-image");
  const [creationMode, setCreationMode] = useState<"text-to-image" | "image-to-image">("text-to-image");
  const [libraryVersion, setLibraryVersion] = useState(0);
  const [historyVersion, setHistoryVersion] = useState(0);
  const [message, setMessage] = useState("");
  const [includeAdultModels, setIncludeAdultModels] = useState(false);
  const [photoExportBusy, setPhotoExportBusy] = useState(false);
  const [photoExportNotice, setPhotoExportNotice] = useState("");
  const [videoFrameReady, setVideoFrameReady] = useState(false);
  const [frameCaptureBusy, setFrameCaptureBusy] = useState(false);
  const [duration, setDuration] = useState(0);
  const [playhead, setPlayhead] = useState(0);
  const [trimIn, setTrimIn] = useState(0);
  const [trimOut, setTrimOut] = useState<number | null>(null);
  const [status, setStatus] = useState<RuntimeStatus | null>(null);
  const [failedIds, setFailedIds] = useState<ReadonlySet<string>>(new Set());
  // Photos the shared canvas editor can't take (over the edit limits, header/decode mismatch) stay
  // viewable read-only, as in PR #30, instead of becoming "Can't preview". Value: the reason shown.
  const [readOnlyPhotos, setReadOnlyPhotos] = useState<ReadonlyMap<string, string>>(new Map());
  const inputRef = useRef<HTMLInputElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const frameCaptureBusyRef = useRef(false);
  const objectUrls = useRef<string[]>([]);
  const activeProjectRef = useRef<string | null>(null);
  activeProjectRef.current = project?.id ?? null;
  const photosClient = projectPhotosClient(invoke);
  const editorItems = mediaForEditor(items, view);
  const selectedId = view === "video" ? selectedVideoId : selectedPhotoId;
  const selected = editorItems.find((item) => item.id === selectedId) ?? null;
  const editHistory = selected?.kind === "image" ? photoHistories[selected.id] : undefined;
  const edits = editHistory?.present;
  const brightness = edits?.brightness ?? 100;
  const contrast = edits?.contrast ?? 100;
  const saturation = edits?.saturation ?? 100;
  const rotation = edits?.rotation ?? 0;
  const straightenTenths = edits?.straightenTenths ?? 0;
  const resizeLongEdge = edits?.resizeLongEdge;
  const flipHorizontal = edits?.flipHorizontal ?? false;
  const flipVertical = edits?.flipVertical ?? false;
  const crop = edits?.crop ?? EMPTY_PHOTO_CROP;
  const selectedPhotoDimensions = selected?.kind === "image" ? photoDimensions[selected.id] : undefined;
  const naturalOutput = selectedPhotoDimensions ? photoResizedOutputDimensions(
    selectedPhotoDimensions.width, selectedPhotoDimensions.height, rotation, crop) : null;
  const resizedOutput = selectedPhotoDimensions ? photoResizedOutputDimensions(
    selectedPhotoDimensions.width, selectedPhotoDimensions.height, rotation, crop, resizeLongEdge) : null;
  const naturalLongestEdge = naturalOutput ? Math.max(naturalOutput.width, naturalOutput.height) : 32;
  const previewFailed = selected ? failedIds.has(selected.id) : false;
  const readOnlyReason = selected ? readOnlyPhotos.get(selected.id) : undefined;
  const editablePhoto = selected?.kind === "image" && !readOnlyReason &&
    ["image/png", "image/jpeg", "image/webp"].includes(selected.mimeType);
  const isVideo = selected?.kind === "video" && !previewFailed;
  function markFailed(id: string, text: string) {
    setFailedIds((previous) => new Set(previous).add(id));
    setMessage(text);
  }

  useEffect(() => {
    void invoke<RuntimeStatus>("runtime_status")
      .then(setStatus)
      .catch(() => setStatus(null));
    return () => {
      objectUrls.current.forEach((url) => URL.revokeObjectURL(url));
      objectUrls.current = [];
    };
  }, []);

  useEffect(() => {
    const projectId = project?.id;
    let active = true;
    setSavedProjectPhotos([]);
    setSavedPhotoProblems([]);
    setProjectPhotoNotice("");
    if (projectId) {
      void photosClient.list(projectId).then((response) => {
        if (!active || activeProjectRef.current !== projectId) return;
        if (response.ok) {
          setSavedProjectPhotos(response.value.photos);
          setSavedPhotoProblems(response.value.problems);
        } else {
          setProjectPhotoNotice("Could not list saved project photos: " + response.error);
        }
      });
    }
    return () => { active = false; };
  }, [project?.id]);

  useEffect(() => {
    if (view === "create") setCreateOpened(true);
    if (view === "create-video") setVideoCreateOpened(true);
    if (view === "enhance-video") setVideoEnhanceOpened(true);
  }, [view]);
  useEffect(() => { if (view !== "video") setVideoFrameReady(false); }, [view]);

  useEffect(() => {
    if (view !== "photo" || !selectedPhotoId || !editablePhoto || photoExportBusy) return;
    const id = selectedPhotoId;
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey || event.isComposing) return;
      const element = event.target;
      if (element instanceof HTMLElement &&
          (element.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(element.tagName))) return;
      const key = event.key.toLowerCase();
      if (key !== "z" && key !== "y") return;
      if (key === "y" && event.shiftKey) return;
      const redo = key === "y" || event.shiftKey;
      event.preventDefault();
      setPhotoHistories((previous) => {
        const history = previous[id];
        if (!history) return previous;
        const next = redo ? redoPhotoEdit(history) : undoPhotoEdit(history);
        return next === history ? previous : { ...previous, [id]: next };
      });
      setPhotoExportNotice("");
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [view, selectedPhotoId, editablePhoto, photoExportBusy]);

  function browseModelsFor(capability: Capability) {
    setModelBrowseCapability(capability);
    setView("models");
  }

  function selectItem(item: MediaItem) {
    if (item.kind === "image") {
      setSelectedPhotoId(item.id);
    } else {
      if (selectedVideoId !== item.id) {
        setVideoFrameReady(false);
        setDuration(0);
        setPlayhead(0);
        setTrimIn(0);
        setTrimOut(null);
      }
      setSelectedVideoId(item.id);
    }
    setPhotoExportNotice("");
    setMessage("");
    if (selectedPhotoId) {
      setPhotoHistories((previous) => {
        const history = previous[selectedPhotoId];
        if (!history) return previous;
        const next = finishPhotoGesture(history);
        return next === history ? previous : { ...previous, [selectedPhotoId]: next };
      });
    }
  }

  function openMedia(item: MediaItem) {
    selectItem(item);
    setView(editorFor(item.kind));
  }

  function updatePhotoAdjustments(changes: Partial<PhotoAdjustments>, gesture: string | null = null) {
    if (!selected || selected.kind !== "image" || photoExportBusy) return;
    const id = selected.id;
    setPhotoHistories((previous) => {
      const history = previous[id] ?? createPhotoHistory();
      const next = applyPhotoEdit(history, changes, gesture);
      return next === history ? previous : { ...previous, [id]: next };
    });
    setPhotoExportNotice("");
  }

  function updatePhotoCrop(changes: Partial<PhotoCrop>, gesture: string | null = null) {
    if (!selected || selected.kind !== "image" || photoExportBusy) return;
    const id = selected.id;
    setPhotoHistories((previous) => {
      const history = previous[id] ?? createPhotoHistory();
      const crop = validatePhotoCrop({ ...EMPTY_PHOTO_CROP, ...history.present.crop, ...changes });
      const next = applyPhotoEdit(history, { crop }, gesture);
      return next === history ? previous : { ...previous, [id]: next };
    });
    setPhotoExportNotice("");
  }

  function finishPhotoSlider() {
    if (!selected || selected.kind !== "image") return;
    const id = selected.id;
    setPhotoHistories((previous) => {
      const history = previous[id];
      if (!history) return previous;
      const next = finishPhotoGesture(history);
      return next === history ? previous : { ...previous, [id]: next };
    });
  }

  function stepPhotoHistory(direction: "undo" | "redo") {
    if (!selected || selected.kind !== "image" || photoExportBusy) return;
    const id = selected.id;
    setPhotoHistories((previous) => {
      const history = previous[id];
      if (!history) return previous;
      const next = direction === "undo" ? undoPhotoEdit(history) : redoPhotoEdit(history);
      return next === history ? previous : { ...previous, [id]: next };
    });
    setPhotoExportNotice("");
  }

  function applyCropAspect(ratio: number) {
    if (!selectedPhotoDimensions) return;
    updatePhotoAdjustments({
      crop: centeredPhotoCrop(selectedPhotoDimensions.width, selectedPhotoDimensions.height, rotation, ratio),
    });
  }

  function addMedia(event: ChangeEvent<HTMLInputElement>) {
    const files = Array.from(event.currentTarget.files ?? []);
    event.currentTarget.value = "";
    const accepted: MediaItem[] = [];
    for (const file of files.slice(0, 50)) {
      const kind = file.type.startsWith("video/") ? "video" : file.type.startsWith("image/") ? "image" : null;
      if (!kind) continue;
      const url = URL.createObjectURL(file);
      objectUrls.current.push(url);
      accepted.push({ id: crypto.randomUUID(), name: file.name, kind, url, size: file.size, mimeType: file.type });
    }
    if (accepted.length > 0) {
      setItems((previous) => [...previous, ...accepted]);
      const firstPhoto = accepted.find((item) => item.kind === "image");
      const firstVideo = accepted.find((item) => item.kind === "video");
      if (firstPhoto && (!selectedPhotoId || view === "photo")) setSelectedPhotoId(firstPhoto.id);
      if (firstVideo && (!selectedVideoId || view === "video")) {
        setSelectedVideoId(firstVideo.id);
        setVideoFrameReady(false);
        setDuration(0); setPlayhead(0); setTrimIn(0); setTrimOut(null);
      }
      setMessage("Media is available this session only. Project media persistence is not implemented yet.");
    } else {
      setMessage("No supported image or video files were selected.");
    }
  }

  function addGenerated(result: { name: string; url: string; size: number }) {
    objectUrls.current.push(result.url);
    const item: MediaItem = { id: crypto.randomUUID(), name: result.name, kind: "image", url: result.url, size: result.size, mimeType: "image/png" };
    setItems((previous) => [...previous, item]);
    setSelectedPhotoId(item.id);
    setView("photo");
    setMessage("Generated image opened in this session. Its file and provenance stay in Optic Rift's output folder; use Save to Pictures to keep a copy elsewhere.");
  }

  async function savePhotoToProject() {
    const source = selected;
    const targetProject = project;
    if (!source || source.kind !== "image" || !targetProject ||
        source.mimeType !== "image/png" || !editablePhoto ||
        photoExportBusy || projectPhotoBusy) return;
    setProjectPhotoBusy(true);
    setProjectPhotoNotice("Verifying and saving a local project-owned PNG copy…");
    try {
      const managed = source.projectPhoto?.projectId === targetProject.id ? source.projectPhoto : undefined;
      const response = await photosClient.save({
        projectId: targetProject.id, assetId: managed?.assetId, expectedRevision: managed?.revision,
        sourceName: source.name, sourceUrl: source.url,
        edits: edits ?? createPhotoHistory().present,
      });
      if (!response.ok) {
        setProjectPhotoNotice("Project photo not saved: " + response.error);
        return;
      }
      const doc = response.value;
      setItems((previous) => previous.map((item) =>
        item.id === source.id ? {
          ...item,
          projectPhoto: { projectId: doc.project_id, assetId: doc.asset_id, revision: doc.revision },
        } : item
      ));
      if (activeProjectRef.current === targetProject.id) {
        const refreshed = await photosClient.list(targetProject.id);
        if (refreshed.ok) {
          setSavedProjectPhotos(refreshed.value.photos);
          setSavedPhotoProblems(refreshed.value.problems);
        } else {
          setProjectPhotoNotice("Photo saved, but project listing failed: " + refreshed.error);
          return;
        }
      }
      setProjectPhotoNotice(`Saved PNG and edit recipe in project "${targetProject.name}" (revision ${doc.revision}).`);
    } catch (error) {
      setProjectPhotoNotice("Project save did not complete: " + (error instanceof Error ? error.message : String(error)));
    } finally {
      setProjectPhotoBusy(false);
    }
  }

  async function openProjectPhoto(assetId: string) {
    const targetProject = project;
    if (!targetProject || projectPhotoBusy) return;
    const already = items.find((item) => item.projectPhoto?.projectId === targetProject.id &&
      item.projectPhoto.assetId === assetId);
    if (already) { openMedia(already); return; }
    setProjectPhotoBusy(true);
    setProjectPhotoNotice("Verifying saved PNG and edit document…");
    let newUrl: string | null = null;
    try {
      const response = await photosClient.open(targetProject.id, assetId);
      if (!response.ok) {
        setProjectPhotoNotice("Could not reopen photo: " + response.error);
        return;
      }
      const doc = response.value.document;
      if (doc.project_id !== targetProject.id || doc.asset_id !== assetId) {
        throw new Error("Returned project photo identity does not match.");
      }
      const restoredEdits = editsFromRecipe(doc.edits);
      const blob = restoredPngBlob(response.value.source_png_base64);
      newUrl = URL.createObjectURL(blob);
      // Reapply normal PNG header/decode limits before the restored source is
      // exposed to the editor. Native checked SHA-256 and source dimensions.
      const check = await loadPhotoBitmap(newUrl);
      check.bitmap.close();
      if (activeProjectRef.current !== targetProject.id) {
        throw new Error("The active project changed while opening the photo.");
      }
      const id = crypto.randomUUID();
      objectUrls.current.push(newUrl);
      const item: MediaItem = {
        id, kind: "image", name: doc.source_name, url: newUrl,
        size: blob.size, mimeType: "image/png",
        projectPhoto: { projectId: doc.project_id, assetId: doc.asset_id, revision: doc.revision },
      };
      newUrl = null;
      setItems((previous) => [...previous, item]);
      setPhotoHistories((previous) => ({
        ...previous, [id]: { past: [], present: restoredEdits, future: [], gesture: null },
      }));
      setSelectedPhotoId(id);
      setView("photo");
      setProjectPhotoNotice(`Reopened "${doc.source_name}" at saved edit revision ${doc.revision}.`);
    } catch (error) {
      if (newUrl) URL.revokeObjectURL(newUrl);
      setProjectPhotoNotice("Could not reopen project photo: " + (error instanceof Error ? error.message : String(error)));
    } finally {
      setProjectPhotoBusy(false);
    }
  }

  async function exportAdjustedPhoto() {
    if (!selected || !editablePhoto || previewFailed || photoExportBusy) return;
    // Pictures can be redirected to OneDrive. Deliberately ask before even
    // preparing the copy; no silent export or photo upload from this workflow.
    const allowed = window.confirm(
      "Export an adjusted PNG copy to Pictures / Optic Rift? Windows may sync Pictures with OneDrive. This does not save a project asset, and the original image stays unchanged."
    );
    if (!allowed) return;
    setPhotoExportBusy(true);
    setPhotoExportNotice("Rendering and validating your edited photo…");
    try {
      const png = await renderAdjustedPhoto(selected.url, { brightness, contrast, saturation, rotation, straightenTenths, resizeLongEdge, crop, flipHorizontal, flipVertical });
      const receipt = await invoke<{ path: string; sha256: string; width: number; height: number }>("photo_export_copy", {
        request: {
          ...png, brightness, contrast, rotation,
          saturation_percent: saturation,
          straighten_tenths: straightenTenths,
          resize_long_edge: resizeLongEdge ?? null,
          crop: { ...crop },
          flip_horizontal: flipHorizontal,
          flip_vertical: flipVertical,
          accept_pictures_export: true,
        },
      });
      setPhotoExportNotice(`Saved adjusted PNG copy: ${receipt.path}. Your original was not changed.`);
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      setPhotoExportNotice(`Export not completed: ${reason}`);
    } finally {
      setPhotoExportBusy(false);
    }
  }

  async function captureCurrentVideoFrame() {
    const source = selected;
    const video = videoRef.current;
    if (frameCaptureBusyRef.current || !source || source.kind !== "video" ||
        view !== "video" || !video || !videoFrameReady || previewFailed) return;
    frameCaptureBusyRef.current = true;
    setFrameCaptureBusy(true);
    try {
      const frame = await captureVideoFrame(video, source.url);
      // A late PNG encoder must never move the user to Photo Studio or reveal
      // a different source after navigating away or switching video clips.
      if (videoRef.current !== video) return;
      const url = URL.createObjectURL(frame.blob);
      const item: MediaItem = {
        id: crypto.randomUUID(),
        name: capturedFrameName(frame.timeSeconds),
        kind: "image",
        url,
        size: frame.blob.size,
        mimeType: "image/png",
        derivedFrom: {
          kind: "video-frame",
          sourceId: source.id,
          sourceName: source.name,
          timeSeconds: frame.timeSeconds,
        },
      };
      objectUrls.current.push(url);
      setItems((previous) => [...previous, item]);
      setSelectedPhotoId(item.id);
      setView("photo");
      setMessage(`Captured the visible frame at ${formatTime(frame.timeSeconds)} into Photo Studio. The frame is session-only; exporting a copy still requires your approval.`);
    } catch (reason) {
      if (videoRef.current === video) {
        setMessage(reason instanceof Error ? reason.message : "Video frame capture did not complete.");
      }
    } finally {
      frameCaptureBusyRef.current = false;
      setFrameCaptureBusy(false);
    }
  }

  function seek(to: number) {
    const value = Math.min(Math.max(to, 0), duration || 0);
    if (videoRef.current) videoRef.current.currentTime = value;
    setPlayhead(value);
  }

  function setInPoint() {
    setTrimIn(Math.min(playhead, trimOut ?? duration));
  }
  function setOutPoint() {
    setTrimOut(Math.max(playhead, trimIn));
  }
  function removeItem(id: string) {
    const entry = items.find((item) => item.id === id);
    if (!entry) return;
    setItems((previous) => previous.filter((item) => item.id !== id));
    if (selectedPhotoId === id) setSelectedPhotoId(items.find((item) => item.id !== id && item.kind === "image")?.id ?? null);
    if (selectedVideoId === id) {
      setSelectedVideoId(items.find((item) => item.id !== id && item.kind === "video")?.id ?? null);
      setVideoFrameReady(false);
      setDuration(0); setPlayhead(0); setTrimIn(0); setTrimOut(null);
    }
    setPhotoHistories((previous) => {
      const next = { ...previous };
      delete next[id];
      return next;
    });
    setPhotoDimensions((previous) => {
      const next = { ...previous };
      delete next[id];
      return next;
    });
    URL.revokeObjectURL(entry.url);
    objectUrls.current = objectUrls.current.filter((url) => url !== entry.url);
  }

  return (
    <DownloadProvider onInstalled={() => setLibraryVersion((v) => v + 1)}>
    <div className="app">
      <aside className="navigation">
        <div className="brand"><img className="brand-icon-img" src="/branding/optic-rift-approved-256.webp" alt="" /><span>OPTIC <b>RIFT</b><small>VISUAL STUDIO</small></span></div>
        <nav className="primary-navigation" aria-label="Primary navigation">
          {navigation.map((section) => <div className="nav-group" key={section.group}>
            <div className="group-label">{section.group}</div>
            {section.items.map((item) => <button key={item.view} type="button"
              className={view === item.view ? "nav selected" : "nav"}
              aria-current={view === item.view ? "page" : undefined}
              aria-label={item.label} title={item.description} onClick={() => setView(item.view)}>
              <span className="nav-symbol" aria-hidden="true">{item.symbol}</span><span className="nav-label">{item.label}</span>
            </button>)}
          </div>)}
        </nav>
        {/* App-lifetime download status (issue #27) on every destination; Models renders its own, so never twice.
            Below 1200 px the rail is icon-only, so styles.css floats this card beside the rail instead. */}
        {view !== "models" && <DownloadStatus includeAdult={includeAdultModels} compact />}
        <div className="nav-footer">
          <span className="status-dot" /> Local-first
          <small>Pre-alpha architecture preview</small>
        </div>
      </aside>

      <div className="shell">
        <header className="topbar">
          <div className="breadcrumbs">OPTIC RIFT <span>/</span> {routeTitle[view]}</div>
          <div className="top-actions"><button className="project-pill" onClick={() => setView("projects")} title="Choose a project">{project ? "Project: " + project.name : "No project open"}</button><span className="pill">PRE-ALPHA</span><button className="top-button" onClick={() => setView("models")}>Model manager</button></div>
        </header>

        {view === "projects" && <ProjectChooser current={project} onOpened={(opened) => { setProject(opened); setView("library"); }} />}

        <input ref={inputRef} type="file"
          accept={view === "photo" ? "image/*" : view === "video" ? "video/*" : "image/*,video/*"}
          multiple hidden onChange={addMedia} aria-label="Select image and video files" />

        {view === "library" && <SessionMediaLibrary items={items} projectName={project?.name ?? null}
          notification={message} onDismissNotification={() => setMessage("")}
          historyVersion={historyVersion} onImport={() => inputRef.current?.click()}
          onOpen={openMedia} onRemove={removeItem} onGeneratedResult={addGenerated}
          savedPhotos={savedProjectPhotos} savedPhotoProblems={savedPhotoProblems}
          savedPhotoBusy={projectPhotoBusy} savedPhotoNotice={projectPhotoNotice}
          onOpenSavedPhoto={(assetId) => void openProjectPhoto(assetId)} />}

        {/* Create stays mounted once opened, so a running job, its progress/Cancel and the current result
            survive a switch to Photo Studio or another page. Hidden content is not rendered visibly. */}
        {createOpened && <main className="create-workspace" hidden={view !== "create"}>
            <GeneratePanel layout="create" visible={view === "create"} prompt={prompt} onPromptChange={setPrompt} mode={creationMode}
              onModeChange={setCreationMode} historyVersion={historyVersion} projectId={project?.id ?? null}
              libraryVersion={libraryVersion} onOpenModels={() => setView("models")}
              onResult={addGenerated}
              onGenerated={() => setHistoryVersion((v) => v + 1)} />
          </main>}
        {view === "workflows" && <CreativeWorkflowExplorer onNavigate={setView} />}

        {/* These planning workspaces are kept mounted after first visit. This
            preserves a session storyboard or enhancement plan across navigation,
            without falsely claiming project persistence or model execution. */}
        {videoCreateOpened && <VideoShotComposer
          media={items}
          onOpenLibrary={() => setView("library")}
          onOpenVideoStudio={() => setView("video")}
          onBrowseModels={(mode: VideoShotMode) => browseModelsFor(mode)}
          visible={view === "create-video"} />}
        {videoEnhanceOpened && <VideoEnhancePlanner
          media={items}
          onOpenLibrary={() => setView("library")}
          onOpenVideoStudio={() => setView("video")}
          onBrowseModels={() => browseModelsFor("video-to-video")}
          visible={view === "enhance-video"} />}

        {(view === "photo" || view === "video") && <main className={`workspace ${view}-workspace`}>
          <section className="media-panel">
            <div className="section-title"><div><h2>{view === "photo" ? "Photos" : "Video clips"}</h2><p>Opened this session</p></div><span className="count">{editorItems.length}</span></div>
            <button className="import-button" onClick={() => inputRef.current?.click()}>＋ Import {view === "photo" ? "photos" : "video"}</button>
            <div className="media-list">
              {editorItems.length === 0 && <div className="empty-library">No {view === "photo" ? "photos" : "videos"} opened this session.<br /><span>Import media or open it from the Media Library.</span></div>}
              {editorItems.map((item) => <div className={selectedId === item.id ? "media-row active" : "media-row"} key={item.id}>
                <button onClick={() => selectItem(item)} className="media-pick">
                  <span className="media-kind">{item.kind === "video" ? "▶" : "▧"}</span><span className="media-label"><strong title={item.name}>{item.name}</strong><small>{item.kind} · {formatSize(item.size)}{failedIds.has(item.id) ? " · can't preview" : ""}</small></span>
                </button>
                <button className="remove-item" aria-label={"Remove " + item.name} title="Remove from this session" onClick={() => removeItem(item.id)}>×</button>
              </div>)}
            </div>
            <div className="sidebar-note">Session media is not saved into projects yet. <button type="button" className="inline-link" onClick={() => setView("library")}>Open Media Library</button></div>
          </section>

          <section className="studio-center">
            <div className="canvas-header"><div><strong>{view === "photo" ? "Photo Studio" : "Video Studio"}</strong><span>{selected ? selected.name : "No media selected"}</span></div><span className="preview-label">{selected?.kind ?? "READY"}</span></div>
            <div className="canvas">
              {selected && previewFailed && <div className="empty-canvas preview-failed" role="alert"><div className="empty-icon">⚠</div><h1>Can't preview {selected.name}</h1><p>{selected.kind === "image" ? IMAGE_PREVIEW_FAILED : VIDEO_PREVIEW_FAILED}</p></div>}
              {editablePhoto && !previewFailed && <PhotoCanvasPreview key={selected.id}
                sourceUrl={selected.url} label={selected.name}
                brightness={brightness} contrast={contrast} saturation={saturation} rotation={rotation}
                straightenTenths={straightenTenths} resizeLongEdge={resizeLongEdge} crop={crop}
                flipHorizontal={flipHorizontal} flipVertical={flipVertical}
                onDimensions={(dims) => setPhotoDimensions((previous) =>
                  previous[selected.id]?.width === dims.width && previous[selected.id]?.height === dims.height
                    ? previous : { ...previous, [selected.id]: dims })}
                onError={(reason) => setReadOnlyPhotos((previous) => new Map(previous).set(selected.id, reason))} />}
              {selected?.kind === "image" && !editablePhoto && !previewFailed &&
                <img key={selected.id} onError={() => markFailed(selected.id, IMAGE_PREVIEW_FAILED)}
                  src={selected.url} alt={selected.name} />}
              {selected?.kind === "image" && !editablePhoto && !previewFailed &&
                <p className="photo-readonly-note">{readOnlyReason ? `Preview only: ${readOnlyReason}` : "Preview only. PNG, JPEG and WebP are supported for editing and export."}</p>}
              {selected?.kind === "video" && !previewFailed && <video key={selected.id} ref={videoRef}
                src={selected.url} controls preload="metadata" playsInline
                onLoadedMetadata={(event) => { const length = event.currentTarget.duration; setDuration(Number.isFinite(length) ? length : 0); setPlayhead(0); }}
                onLoadedData={() => setVideoFrameReady(true)}
                onSeeking={() => setVideoFrameReady(false)}
                onSeeked={() => setVideoFrameReady(true)}
                onTimeUpdate={(event) => setPlayhead(event.currentTarget.currentTime)}
                onError={() => { setVideoFrameReady(false); markFailed(selected.id, VIDEO_PREVIEW_FAILED); }} />}
              {!selected && <div className="empty-canvas"><div className="empty-icon">◉</div><h1>{view === "photo" ? "Edit a photo." : "Preview a video."}</h1><p>{view === "photo" ? "Open a photo to adjust and export a new PNG copy." : "Open a clip to play, seek, and mark in/out points. Timeline export is not yet available."}</p><button className="primary" onClick={() => inputRef.current?.click()}>Import {view === "photo" ? "a photo" : "a video"}</button></div>}
            </div>
            <div className="player-controls">
              {isVideo ? <><button className="control" onClick={() => seek(playhead - 0.1)} title="Step back 0.1 seconds">↶ 0.1s</button><button className="control" onClick={() => seek(playhead + 0.1)} title="Step forward 0.1 seconds">0.1s ↷</button><span className="time">{formatTime(playhead)} / {formatTime(duration)}</span></> :
                <span className="time">{previewFailed ? "Preview unavailable" : selected ? "Image preview" : "Import media to begin"}</span>}
              <span className="control-hint">Originals never modified</span>
            </div>
            {view === "video" && <div className="timeline-panel"><div className="timeline-heading"><strong>Timeline preview</strong><span>Non-destructive markers only</span></div>
              {isVideo ? <>
                <input className="scrubber" type="range" aria-label="Scrub video" min={0} max={Math.max(duration, 0.01)} step={0.01} value={Math.min(playhead, duration || 0)} onChange={(e) => seek(Number(e.target.value))} />
                <div className="trim-tools"><button onClick={setInPoint}>Mark in: {formatTime(trimIn)}</button><button onClick={setOutPoint}>Mark out: {trimOut === null ? "end" : formatTime(trimOut)}</button><span>Preview markers only; exporting trims is not available yet.</span></div>
                <div className="timeline-clip"><span>▶</span><strong>{selected?.name}</strong></div>
              </> : <div className="timeline-empty">Select a video to see its clip timeline.</div>}
            </div>}
          </section>

          <aside className="inspector editor-inspector">
            <div className="section-title"><div><h2>{view === "photo" ? "Photo adjustments" : "Video tools"}</h2><p>{view === "photo" ? "Original preserved; export creates a copy" : "Playback and non-destructive markers"}</p></div></div>
            {view === "photo" && editablePhoto && !previewFailed ? <div className="adjustments">
              <h3>Image preview</h3>
              <div className="photo-history-actions" role="group" aria-label="Photo edit history">
                <button type="button" className="secondary" disabled={photoExportBusy || !editHistory?.past.length}
                  title="Undo last edit (Ctrl+Z)" onClick={() => stepPhotoHistory("undo")}>Undo</button>
                <button type="button" className="secondary" disabled={photoExportBusy || !editHistory?.future.length}
                  title="Redo edit (Ctrl+Y or Ctrl+Shift+Z)" onClick={() => stepPhotoHistory("redo")}>Redo</button>
                <button type="button" className="secondary" disabled={photoExportBusy ||
                  (brightness === 100 && contrast === 100 && saturation === 100 && rotation === 0 && straightenTenths === 0 &&
                   !flipHorizontal && !flipVertical && Object.values(crop).every((n) => n === 0))}
                  onClick={() => updatePhotoAdjustments({ ...DEFAULT_PHOTO_EDITS, crop: { ...EMPTY_PHOTO_CROP },
                    flipHorizontal: false, flipVertical: false, resizeLongEdge: undefined })}>Reset all</button>
              </div>
              <p className="photo-history-note">Up to 40 undo steps per photo, this session only. These edits are not saved in your project.</p>
              {selected?.derivedFrom?.kind === "video-frame" &&
                <p className="photo-frame-origin">
                  Session-only frame from <strong>{selected.derivedFrom.sourceName}</strong> at {formatTime(selected.derivedFrom.timeSeconds)}.
                  This source relationship is not saved with the exported PNG.
                </p>}
              <label>Brightness <output>{brightness}%</output><input type="range" min="25" max="175" value={brightness} disabled={photoExportBusy}
                onChange={(event) => updatePhotoAdjustments({ brightness: Number(event.target.value) }, "brightness")}
                onPointerUp={finishPhotoSlider} onPointerCancel={finishPhotoSlider} onKeyUp={finishPhotoSlider} onBlur={finishPhotoSlider} /></label>
              <label>Contrast <output>{contrast}%</output><input type="range" min="25" max="175" value={contrast} disabled={photoExportBusy}
                onChange={(event) => updatePhotoAdjustments({ contrast: Number(event.target.value) }, "contrast")}
                onPointerUp={finishPhotoSlider} onPointerCancel={finishPhotoSlider} onKeyUp={finishPhotoSlider} onBlur={finishPhotoSlider} /></label>
              <label>Saturation <output>{saturation}%</output><input type="range" min="0" max="200" step="1"
                value={saturation} disabled={photoExportBusy} aria-label="Photo saturation percent"
                onChange={(event) => updatePhotoAdjustments({ saturation: Number(event.target.value) }, "saturation")}
                onPointerUp={finishPhotoSlider} onPointerCancel={finishPhotoSlider}
                onKeyUp={finishPhotoSlider} onBlur={finishPhotoSlider} /></label>
              <button className="secondary full" disabled={photoExportBusy}
                onClick={() => updatePhotoAdjustments({ rotation: (rotation + 90) % 360 })}>Rotate photo 90°</button>
              <label>Fine straighten <output>{(straightenTenths / 10).toFixed(1)}°</output>
                <input type="range" min="-150" max="150" step="5" value={straightenTenths}
                  aria-label="Fine straighten degrees clockwise" disabled={photoExportBusy}
                  onChange={(event) => updatePhotoAdjustments({ straightenTenths: Number(event.target.value) }, "straighten")}
                  onPointerUp={finishPhotoSlider} onPointerCancel={finishPhotoSlider}
                  onKeyUp={finishPhotoSlider} onBlur={finishPhotoSlider} /></label>
              <p className="control-hint">Levels by up to 15°. The image automatically zooms to cover empty corners and may crop its edges.</p>
              <div className="photo-flip-actions" role="group" aria-label="Flip image">
                <button type="button" className="secondary" aria-pressed={flipHorizontal} disabled={photoExportBusy}
                  onClick={() => updatePhotoAdjustments({ flipHorizontal: !flipHorizontal })}>Flip horizontal</button>
                <button type="button" className="secondary" aria-pressed={flipVertical} disabled={photoExportBusy}
                  onClick={() => updatePhotoAdjustments({ flipVertical: !flipVertical })}>Flip vertical</button>
              </div>
              <fieldset className="photo-resize-tools">
                <legend>Resize exported photo</legend>
                <p>Keep the original aspect ratio. Downsize only, never stretch or invent pixels.</p>
                <div className="photo-resize-presets" role="group" aria-label="Resize presets">
                  <button type="button" className="secondary" disabled={photoExportBusy || resizeLongEdge === undefined}
                    onClick={() => updatePhotoAdjustments({ resizeLongEdge: undefined })}>Original</button>
                  <button type="button" className="secondary" disabled={photoExportBusy || !naturalOutput || naturalLongestEdge < 64}
                    onClick={() => updatePhotoAdjustments({ resizeLongEdge: Math.round(naturalLongestEdge / 2) })}>Half</button>
                  <button type="button" className="secondary" disabled={photoExportBusy || !naturalOutput || naturalLongestEdge < 128}
                    onClick={() => updatePhotoAdjustments({ resizeLongEdge: Math.round(naturalLongestEdge / 4) })}>Quarter</button>
                </div>
                <label>Longest edge <output>{resizedOutput ? Math.max(resizedOutput.width, resizedOutput.height) : "—"} px</output>
                  <input type="range" min="32" max={Math.max(32, naturalLongestEdge)} step="1"
                    value={Math.min(resizeLongEdge ?? naturalLongestEdge, naturalLongestEdge)} aria-label="Output longest edge pixels"
                    disabled={photoExportBusy || !naturalOutput || naturalLongestEdge < 32}
                    onChange={(event) => updatePhotoAdjustments({ resizeLongEdge: Number(event.target.value) }, "resize")}
                    onPointerUp={finishPhotoSlider} onPointerCancel={finishPhotoSlider}
                    onKeyUp={finishPhotoSlider} onBlur={finishPhotoSlider} /></label>
                <p role="status">PNG output: {resizedOutput ? `${resizedOutput.width} × ${resizedOutput.height} px` : "Preparing image dimensions…"}</p>
              </fieldset>
              <button className="primary full" type="button" disabled={photoExportBusy}
                onClick={() => void exportAdjustedPhoto()}>
                {photoExportBusy ? "Saving adjusted PNG…" : "Save adjusted PNG copy"}
              </button>
              {photoExportNotice && <p role="status" className="photo-export-notice">{photoExportNotice}</p>}
              {project && <div className="project-photo-actions">
                <button type="button" className="secondary full"
                  disabled={photoExportBusy || projectPhotoBusy || selected?.mimeType !== "image/png"}
                  onClick={() => void savePhotoToProject()}>
                  {projectPhotoBusy ? "Saving project PNG…" :
                    selected?.projectPhoto?.projectId === project.id
                      ? "Save new edit revision to project" : "Save PNG and edits to project"}
                </button>
                <p>Explicit local managed copy. PNG only. Project data stays on this computer;
                  not a portable bundle, vault, or automatic save.</p>
                {projectPhotoNotice && <p role="status">{projectPhotoNotice}</p>}
              </div>}

              <fieldset className="photo-crop-tools">
                <legend>Crop photo</legend>
                <p>Crop the rotated image. The preview and exported PNG use the same cropped pixels; the original is unchanged.</p>
                <div className="photo-crop-presets" role="group" aria-label="Crop aspect presets">
                  <button type="button" className="secondary" disabled={!selectedPhotoDimensions || photoExportBusy} onClick={() => applyCropAspect(1)}>Square 1:1</button>
                  <button type="button" className="secondary" disabled={!selectedPhotoDimensions || photoExportBusy} onClick={() => applyCropAspect(16 / 9)}>Wide 16:9</button>
                  <button type="button" className="secondary" disabled={!selectedPhotoDimensions || photoExportBusy} onClick={() => applyCropAspect(4 / 5)}>Portrait 4:5</button>
                  <button type="button" className="secondary" disabled={photoExportBusy}
                    onClick={() => updatePhotoAdjustments({ crop: { ...EMPTY_PHOTO_CROP } })}>Reset crop</button>
                </div>
                {(["left", "top", "right", "bottom"] as const).map((edge) => {
                  const opposite = edge === "left" ? "right" : edge === "right" ? "left" :
                    edge === "top" ? "bottom" : "top";
                  const max = Math.min(9500, 9900 - crop[opposite]);
                  return <label key={edge} className="photo-crop-control">
                    {edge.charAt(0).toUpperCase() + edge.slice(1)} edge <output>{(crop[edge] / 100).toFixed(2)}%</output>
                    <input type="range" min={0} max={max} step={25} value={crop[edge]} disabled={photoExportBusy}
                      aria-label={edge + " crop inset percent"}
                      onChange={(event) => updatePhotoCrop({ ...crop, [edge]: Number(event.target.value) }, `crop-${edge}`)}
                      onPointerUp={finishPhotoSlider} onPointerCancel={finishPhotoSlider}
                      onKeyUp={finishPhotoSlider} onBlur={finishPhotoSlider} />
                  </label>;
                })}
                <p className="photo-crop-status" role="status">
                  {crop.left + crop.top + crop.right + crop.bottom === 0
                    ? "Full photo, no crop."
                    : "Crop preview active. Export saves only the visible area."}
                </p>
              </fieldset>
              <p>Export to Pictures may sync with OneDrive. The original stays unchanged. Supports PNG, JPEG and WebP sources up to 32 MB and 20 megapixels. PNG export does not preserve source EXIF or ICC metadata, and it does not save an asset into this project.</p>
            </div> : view === "photo" ? <p className="model-hint">Choose a photo to adjust and export, or use Create Images for AI generation.</p> :
              <div className="video-capability-note">
                <strong>Video frame to photo</strong>
                <p>Pause or seek to a frame, then open a PNG copy in Photo Studio. No model, audio processing, file export, or cloud upload is involved.</p>
                <button type="button" className="primary full" onClick={() => void captureCurrentVideoFrame()}
                  disabled={!isVideo || !videoFrameReady || frameCaptureBusy}
                  title={!videoFrameReady ? "Wait for the current video frame to load" : "Capture this frame into Photo Studio"}>
                  {frameCaptureBusy ? "Capturing frame…" : "Capture frame to Photo Studio"}
                </button>
                <p>This frame is held only for this session. The original video is unchanged. Use Photo Studio's separate Save action to export a PNG.</p>
                <strong>Video timeline editing is not yet available.</strong>
                <p>You can play a local clip, seek in 0.1-second steps, and mark an in/out range. Markers are preview-only. Clip assembly, transitions and video export still need qualified native implementation.</p>
                {isVideo && <p>Current preview range: {formatTime(trimIn)} to {formatTime(trimOut ?? duration)}.</p>}
              </div>}
            {view === "photo" && <>
              <button className="secondary full" onClick={() => setView("create")}>Create an image</button>
              <GenerationHistory refreshVersion={historyVersion} onResult={addGenerated} />
            </>}
          </aside>
          {message && <div className="notice" role="status"><span>{message}</span><button onClick={() => setMessage("")} aria-label="Dismiss notification">×</button></div>}
        </main>}

        {view === "models" && <main className="content-page"><div className="page-eyebrow">MODEL LIBRARY</div><h1>Find the right tool for your idea.</h1><p className="lead">Discover image and video models on Hugging Face, see what they do, what hardware they need and how they are licensed, and install the ones Optic Rift can run on this computer.</p>
          <ModelBrowser includeAdult={includeAdultModels} initialCapability={modelBrowseCapability} onLibraryChanged={() => setLibraryVersion((v) => v + 1)} />
          <HuggingFaceAccess />
          <div className="adult-preference">
            <div className="preference-top"><div><strong>Adult-oriented model discovery</strong><p>Models designed to create explicit sexual content will be labeled <b>Adult-focused (18+) · Explicit</b>. They will be excluded from discovery unless you opt in.</p></div><span className="adult-tag">18+</span></div>
            <label className="adult-toggle">
              <input type="checkbox" checked={includeAdultModels} onChange={(event) => {
                if (!event.target.checked) { setIncludeAdultModels(false); return; }
                const allowed = window.confirm("Adult-focused models can produce sexually explicit material. Confirm you are 18 or older and want to include their listings. Explicit previews will remain hidden until you choose to reveal them.");
                setIncludeAdultModels(allowed);
              }} />
              Include adult-focused models in discovery
            </label>
            <p className="preference-caption" role="status">{includeAdultModels ? "Opted in for this session. Explicit previews will still be hidden until opened intentionally." : "Off by default. This does not hide or alter media you've imported yourself."}</p>
            <p className="preference-caption">Session-only preference. Listings flagged "not for all audiences" by their source are shown with an 18+ badge; preview images are never loaded. This is not age verification, and unflagged models are not certified as general-audience.</p>
          </div>
          <p className="disclosure">Discovery is broad; execution is qualified. Search is anonymous. Installed models run in a separate local process with no network access.</p></main>}

        {view === "settings" && <main className="content-page"><div className="page-eyebrow">PREFERENCES</div><h1>Your studio stays on your computer.</h1><p className="lead">Local files, user-controlled AI models, and clear permission boundaries come first.</p><div className="setting-row"><span><strong>Application shell</strong><small>{status?.stage ?? "Browser preview: native Rust bridge unavailable"}</small></span><strong>{status ? "Tauri / Rust" : "Browser-only preview"}</strong></div><div className="setting-row"><span><strong>Model execution</strong><small>Local text-to-image through an isolated Python worker; set up on the Models page</small></span><strong>Text to image</strong></div><div className="setting-row"><span><strong>Persistent projects</strong><small>Named projects are saved locally and reopen after restart. Imported media is not saved into projects yet.</small></span><strong>Names only</strong></div><div className="setting-row"><span><strong>Local/cloud policy</strong><small>No cloud generation is enabled or silently selected</small></span><strong>Local first</strong></div></main>}
      </div>
    </div>
    </DownloadProvider>
  );
}
