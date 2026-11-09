import { useEffect, useRef, useState } from "react";
import { loadPhotoBitmap, paintAdjustedPhoto, type PhotoCrop } from "./photoEdits";

type Props = {
  sourceUrl: string;
  label: string;
  brightness: number;
  contrast: number;
  rotation: number;
  crop?: PhotoCrop;
  onDimensions?: (dimensions: { width: number; height: number }) => void;
  onError: (message: string) => void;
};

/**
 * A bounded proxy rendered with the exact same canvas filter/rotation pipeline
 * as Save adjusted PNG. Decode once per selected photo, repaint on slider moves.
 * No intermediate PNG files or URLs are written to disk.
 */
export default function PhotoCanvasPreview({
  sourceUrl, label, brightness, contrast, rotation, crop, onDimensions, onError,
}: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const onErrorRef = useRef(onError);
  const onDimensionsRef = useRef(onDimensions);
  onErrorRef.current = onError;
  onDimensionsRef.current = onDimensions;

  const [decoded, setDecoded] = useState<{ sourceUrl: string; bitmap: ImageBitmap } | null>(null);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    let active = true;
    let ownedBitmap: ImageBitmap | null = null;
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
    // Debounce rapid slider changes to avoid repeatedly rasterizing 20 MP
    // inputs. The live proxy is capped at 1400 px; full-resolution export
    // uses the same renderer but without the proxy bound.
    const handle = window.setTimeout(() => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      try {
        paintAdjustedPhoto(decoded.bitmap, { brightness, contrast, rotation, crop }, canvas, 1400);
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
  }, [decoded, sourceUrl, brightness, contrast, rotation, crop?.left, crop?.top, crop?.right, crop?.bottom]);

  const shown = ready && decoded?.sourceUrl === sourceUrl && !error;
  return <>
    <canvas
      ref={canvasRef}
      className="photo-preview-canvas"
      role="img"
      aria-label={label}
      style={{ visibility: shown ? "visible" : "hidden" }}
    />
    {!shown && !error && <div className="photo-preview-status" role="status">Preparing photo preview…</div>}
    {error && <p className="photo-preview-status" role="alert">{error}</p>}
  </>;
}
