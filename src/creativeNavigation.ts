/** User-intent navigation, not a list of model backends.
 * Only real destinations ship here. Video is explicitly preview/markers only
 * until the native timeline/export pipeline is qualified.
 */
export type View = "projects" | "library" | "create" | "photo" | "video" | "models" | "settings";
export type MediaKind = "image" | "video";
export type MediaItem = {
  id: string;
  name: string;
  kind: MediaKind;
  url: string;
  size: number;
  mimeType: string;
};

export const navigation = [
  { group: "YOUR WORK", items: [
    { view: "projects", label: "Projects", symbol: "▤", description: "Open or create a local project" },
    { view: "library", label: "Media Library", symbol: "▦", description: "Opened media and verified generated outputs" },
  ] },
  { group: "CREATE & EDIT", items: [
    { view: "create", label: "Create Images", symbol: "✦", description: "Generate images with a qualified local model" },
    { view: "photo", label: "Photo Studio", symbol: "▧", description: "Adjust and export photos" },
    { view: "video", label: "Video Studio", symbol: "▶", description: "Preview video and mark in/out, editing export not yet available" },
  ] },
  { group: "TOOLS", items: [
    { view: "models", label: "Models", symbol: "◈", description: "Discover and install supported model configurations" },
    { view: "settings", label: "Settings", symbol: "⚙", description: "Local settings and privacy" },
  ] },
] as const satisfies ReadonlyArray<{
  group: string; items: ReadonlyArray<{ view: View; label: string; symbol: string; description: string }>;
}>;

export const routeTitle: Record<View, string> = {
  projects: "Projects", library: "Media Library", create: "Create Images",
  photo: "Photo Studio", video: "Video Studio", models: "Models", settings: "Settings",
};

export const editorFor = (kind: MediaKind): "photo" | "video" => kind === "image" ? "photo" : "video";

/** Viewing the appropriate kind is an explicit choice. Never show another
 * format in the editor simply because that was selected on a different route.
 */
export function mediaForEditor(items: readonly MediaItem[], view: View): MediaItem[] {
  if (view === "photo") return items.filter((item) => item.kind === "image");
  if (view === "video") return items.filter((item) => item.kind === "video");
  return [];
}
