import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { loadPhotoBitmap, paintAdjustedPhoto, type PhotoCrop } from "./photoEdits";
import { photoViewportFit, photoClampPan, photoZoomStep, type PhotoPoint, type PhotoSize } from "./photoViewport";

type Props = {
  sourceUrl: string;
  label: string;
  brightness: number;
  contrast: number;
  rotation: number;
  crop?: PhotoCrop;
  flipHorizontal?: boolean;
  flipVertical?: boolean;
  onDimensions?: (dimensions: { width: number; height: number }) => void;
  onError: (message: string) => void;
};

type Drag = { id: number; startX: number; startY: number; initialPan: PhotoPoint };
const ZERO = { x: 0, y: 0 };
const EMPTY_SIZE: PhotoSize = { width: 0, height: 0 };

/**
 * A bounded preview rendered with the exact same image operations as PNG export.
 * Zoom/pan are viewport-only CSS transforms, not part of the saved edit recipe.
 * Decode once per photo; never store private previews or intermediate files.
 */
export default function PhotoCanvasPreview({
  sourceUrl, label, brightness, contrast, rotation, crop, flipHorizontal, flipVertical, onDimensions, onError,
}: Props) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const dragRef = useRef<Drag | null>(null);
  const onErrorRef = useRef(onError);
  const onDimensionsRef = useRef(onDimensions);
  onErrorRef.current = onError;
  onDimensionsRef.current = onDimensions;

  const [decoded, setDecoded] = useState<{ sourceUrl: string; bitmap: ImageBitmap } | null>(null);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState("");
  const [viewport, setViewport] = useState<PhotoSize>(EMPTY_SIZE);
  const [image, setImage] = useState<PhotoSize>(EMPTY_SIZE);
  const [zoom, setZoom] = useState(1);
  const [showOriginal, setShowOriginal] = useState(false);
  const [pan, setPan] = useState<PhotoPoint>(ZERO);
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    const element = viewportRef.current;
    if (!element) return;
    const update = () => {
      const next = { width: element.clientWidth, height: element.clientHeight };
      setViewport((current) => current.width === next.width && current.height === next.height ? current : next);
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    let active = true;
    let ownedBitmap: ImageBitmap | null = null;
    dragRef.current = null;
    setDragging(false);
    setZoom(1);
    setPan(ZERO);
    setShowOriginal(false);
    setImage(EMPTY_SIZE);
    setDecoded(null);
    setReady(false);
    setError("");
    void loadPhotoBitmap(sourceUrl).then(({ bitmap }) => {
      if (!active) {
        bitmap.close();
        return;
      }
      ownedBitmap = bitmap;
      onDimensionsRef.current?.({ width: bitmap.width, height: bitmap.height });
      setDecoded({ sourceUrl, bitmap });
    }).catch((reason: unknown) => {
      if (!active) return;
      const message = reason instanceof Error ? reason.message : "Could not preview this photo.";
      setError(message);
      onErrorRef.current(message);
    });
    return () => {
      active = false;
      ownedBitmap?.close();
    };
  }, [sourceUrl]);

  useEffect(() => {
    if (!decoded || decoded.sourceUrl !== sourceUrl) {
      setReady(false);
      return;
    }
    // Debounce changes: zoom and pan must not trigger expensive repaints.
    const handle = window.setTimeout(() => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      try {
        // Compare by rendering the immutable source through the *same* bounded
        // renderer, never by changing the active edit recipe or exported PNG.
        const dims = showOriginal
          ? paintAdjustedPhoto(decoded.bitmap, { brightness: 100, contrast: 100, rotation: 0 }, canvas, 1400)
          : paintAdjustedPhoto(decoded.bitmap, { brightness, contrast, rotation, crop, flipHorizontal, flipVertical }, canvas, 1400);
        setImage((old) => old.width === dims.width && old.height === dims.height ? old : dims);
        setReady(true);
        setError("");
      } catch (reason) {
        const message = reason instanceof Error ? reason.message : "Could not render the photo preview.";
        setReady(false);
        setError(message);
        onErrorRef.current(message);
      }
    }, 90);
    return () => window.clearTimeout(handle);
  }, [decoded, sourceUrl, brightness, contrast, rotation, crop?.left, crop?.top, crop?.right, crop?.bottom, flipHorizontal, flipVertical, showOriginal]);

  useEffect(() => {
    setPan((current) => {
      const clamped = photoClampPan(current, image, viewport, zoom);
      return clamped.x === current.x && clamped.y === current.y ? current : clamped;
    });
  }, [image, viewport, zoom]);

  const fitted = photoViewportFit(image, viewport);
  const canCompare = decoded?.sourceUrl === sourceUrl && !error && fitted.width > 0;
  const shown = ready && canCompare;
  const actualPan = photoClampPan(pan, image, viewport, zoom);

  function adjustZoom(direction: -1 | 1) {
    if (!shown) return;
    setZoom((current) => photoZoomStep(current, direction));
  }

  function fitToWindow() {
    dragRef.current = null;
    setDragging(false);
    setZoom(1);
    setPan(ZERO);
  }

  function beginDrag(event: ReactPointerEvent<HTMLDivElement>) {
    if (!shown || zoom <= 1 || event.button !== 0 || event.target !== canvasRef.current) return;
    dragRef.current = {
      id: event.pointerId, startX: event.clientX, startY: event.clientY, initialPan: actualPan,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
    event.currentTarget.focus();
    setDragging(true);
    event.preventDefault();
  }

  function dragPhoto(event: ReactPointerEvent<HTMLDivElement>) {
    const drag = dragRef.current;
    if (!drag || drag.id !== event.pointerId) return;
    setPan(photoClampPan({
      x: drag.initialPan.x + event.clientX - drag.startX,
      y: drag.initialPan.y + event.clientY - drag.startY,
    }, image, viewport, zoom));
  }

  function endDrag(event: ReactPointerEvent<HTMLDivElement>) {
    if (dragRef.current?.id !== event.pointerId) return;
    dragRef.current = null;
    setDragging(false);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  function handleViewportKey(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (!shown || event.target !== event.currentTarget || event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.key === "+" || event.key === "=") adjustZoom(1);
    else if (event.key === "-") adjustZoom(-1);
    else if (event.key === "0" || event.key.toLowerCase() === "f") fitToWindow();
    else if (zoom > 1) {
      const offset: Record<string, PhotoPoint> = {
        ArrowLeft: { x: 48, y: 0 }, ArrowRight: { x: -48, y: 0 },
        ArrowUp: { x: 0, y: 48 }, ArrowDown: { x: 0, y: -48 },
      };
      const step = offset[event.key];
      if (!step) return;
      setPan((old) => photoClampPan({ x: old.x + step.x, y: old.y + step.y }, image, viewport, zoom));
    } else return;
    event.preventDefault();
  }

  return <div ref={viewportRef} className={dragging ? "photo-viewport is-dragging" : "photo-viewport"}
    role="region" tabIndex={shown ? 0 : -1}
    aria-label={`${label} photo inspection viewport, ${Math.round(zoom * 100)} percent zoom`}
    onKeyDown={handleViewportKey} onPointerDown={beginDrag} onPointerMove={dragPhoto}
    onPointerUp={endDrag} onPointerCancel={endDrag} onLostPointerCapture={() => { dragRef.current = null; setDragging(false); }}>
    <canvas ref={canvasRef} className="photo-preview-canvas" role="img" aria-label={label}
      style={{
        visibility: shown ? "visible" : "hidden",
        width: fitted.width > 0 ? `${fitted.width}px` : "100%",
        height: fitted.height > 0 ? `${fitted.height}px` : "100%",
        maxWidth: "none", maxHeight: "none",
        transform: `translate(${actualPan.x}px, ${actualPan.y}px) scale(${zoom})`,
        transformOrigin: "center center",
      }} />
    {!shown && !error && <div className="photo-preview-status" role="status">Preparing photo preview…</div>}
    {error && <p className="photo-preview-status" role="alert">{error}</p>}
    {canCompare && <div className="photo-comparison-toolbar">
      <button type="button" className="photo-compare-toggle" aria-label="Show unedited original"
        aria-pressed={showOriginal} title="Toggle the original photo for comparison"
        onClick={() => { setReady(false); setShowOriginal((current) => !current); }}>
        Original
      </button>
      <span aria-live="polite">{showOriginal ? "Viewing original" : "Viewing edited"}</span>
    </div>}
    {shown && <div className="photo-zoom-toolbar" role="toolbar" aria-label="Photo zoom controls">
      <button type="button" disabled={zoom <= 1} onClick={() => adjustZoom(-1)} aria-label="Zoom out" title="Zoom out (-)">−</button>
      <button type="button" onClick={fitToWindow} aria-label="Fit photo to window" title="Fit photo (0 or F)">Fit</button>
      <output aria-label="Zoom relative to fitted preview" title="Percentage of the fitted preview, not original pixel size">{Math.round(zoom * 100)}%</output>
      <button type="button" disabled={zoom >= 4} onClick={() => adjustZoom(1)} aria-label="Zoom in" title="Zoom in (+)">+</button>
    </div>}
  </div>;
}
