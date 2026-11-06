import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/** Transient, on-demand thumbnail; never writes a copy to local disk. */
export default function GenerationThumbnail({ jobId }: { jobId: string }) {
  const host = useRef<HTMLSpanElement>(null);
  const [visible, setVisible] = useState(false);
  const [uri, setUri] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setVisible(false);
    setUri(null);
    setFailed(false);
    const node = host.current;
    if (!node || typeof IntersectionObserver === "undefined") {
      setVisible(true);
      return;
    }
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        setVisible(true);
        observer.disconnect();
      }
    }, { rootMargin: "120px" });
    observer.observe(node);
    return () => observer.disconnect();
  }, [jobId]);

  useEffect(() => {
    if (!visible) return;
    let active = true;
    void invoke<string>("generation_output_thumbnail", { jobId })
      .then((value) => {
        if (!active) return;
        if (!value.startsWith("data:image/png;base64,") || value.length > 750_000) {
          setFailed(true);
          return;
        }
        setUri(value);
      })
      .catch(() => { if (active) setFailed(true); });
    return () => { active = false; };
  }, [visible, jobId]);

  return <span className="generation-thumbnail" ref={host}>
    {uri ? <img src={uri} alt="" draggable={false} /> : <span aria-hidden="true">{failed ? "!" : "▧"}</span>}
  </span>;
}
