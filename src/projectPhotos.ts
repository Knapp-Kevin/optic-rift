/** Explicitly managed project PNG sources and reversible recipe versions.
 * This prototype does not authorize linked external paths, JPEG/WebP project
 * storage, project portability, automatic saving, or an encrypted vault.
 */
import { loadPhotoBitmap, validatePhotoAdjustments, MAX_PHOTO_SOURCE_BYTES, type PhotoAdjustments } from "./photoEdits.ts";
import { errorText, type Invoke, type Result } from "./projects.ts";

export const PROJECT_PHOTO_RECIPE_VERSION = 1;

export type PhotoRecipe = {
  schema_version: number;
  brightness: number;
  contrast: number;
  saturation: number;
  rotation: number;
  straighten_tenths: number;
  resize_long_edge: number | null;
  crop: { left: number; top: number; right: number; bottom: number } | null;
  flip_horizontal: boolean;
  flip_vertical: boolean;
};

export type ProjectPhotoDocument = {
  schema_version: number;
  project_id: string;
  asset_id: string;
  revision: number;
  source_name: string;
  source_sha256: string;
  source_width: number;
  source_height: number;
  edits: PhotoRecipe;
};
export type ProjectPhotoProblem = { asset_id: string; error: string };
export type ProjectPhotoListing = {
  photos: ProjectPhotoDocument[];
  problems: ProjectPhotoProblem[];
  /** Candidate staging paths only. They are not committed, validated, or authorized for deletion. */
  possible_incomplete_saves: number;
};
export type OpenedProjectPhoto = { document: ProjectPhotoDocument; source_png_base64: string };

export function recipeFromEdits(adjustments: PhotoAdjustments): PhotoRecipe {
  const valid = validatePhotoAdjustments(adjustments);
  return {
    schema_version: PROJECT_PHOTO_RECIPE_VERSION,
    brightness: valid.brightness,
    contrast: valid.contrast,
    saturation: valid.saturation ?? 100,
    rotation: valid.rotation,
    straighten_tenths: valid.straightenTenths ?? 0,
    resize_long_edge: valid.resizeLongEdge ?? null,
    crop: valid.crop ? { ...valid.crop } : null,
    flip_horizontal: valid.flipHorizontal ?? false,
    flip_vertical: valid.flipVertical ?? false,
  };
}

export function editsFromRecipe(recipe: PhotoRecipe): PhotoAdjustments {
  if (!recipe || typeof recipe !== "object") throw new Error("The saved photo edit document is invalid.");
  if (recipe.schema_version !== PROJECT_PHOTO_RECIPE_VERSION) {
    throw new Error("This photo uses an unsupported edit document version.");
  }
  if (!recipe || typeof recipe !== "object" ||
      typeof recipe.flip_horizontal !== "boolean" || typeof recipe.flip_vertical !== "boolean" ||
      typeof recipe.straighten_tenths !== "number" ||
      (recipe.resize_long_edge !== null && typeof recipe.resize_long_edge !== "number")) {
    throw new Error("The saved photo edit document is invalid.");
  }
  return validatePhotoAdjustments({
    brightness: recipe.brightness, contrast: recipe.contrast, saturation: recipe.saturation,
    rotation: recipe.rotation, straightenTenths: recipe.straighten_tenths,
    ...(recipe.resize_long_edge !== null ? { resizeLongEdge: recipe.resize_long_edge } : {}),
    ...(recipe.crop !== null ? { crop: { ...recipe.crop } } : {}),
    flipHorizontal: recipe.flip_horizontal, flipVertical: recipe.flip_vertical,
  });
}

/** Convert a session PNG blob to bounded base64 only after the existing source
 * header/decode verifier accepts it. Never read a filesystem path from JS.
 */
export async function sourcePngBase64(sourceUrl: string): Promise<string> {
  if (!sourceUrl.startsWith("blob:")) throw new Error("Only opened session PNG files can be saved.");
  const checked = await loadPhotoBitmap(sourceUrl);
  checked.bitmap.close();
  const response = await fetch(sourceUrl);
  if (!response.ok) throw new Error("The session photo is no longer available.");
  const blob = await response.blob();
  if (blob.type !== "image/png" || blob.size === 0 || blob.size > MAX_PHOTO_SOURCE_BYTES) {
    throw new Error("Save to Project currently supports PNG originals up to 32 MB.");
  }
  return await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("Could not read the PNG for project storage."));
    reader.onload = () => {
      const value = reader.result;
      if (typeof value !== "string" || !value.startsWith("data:image/png;base64,")) {
        reject(new Error("Photo source was not encoded as PNG."));
        return;
      }
      resolve(value.slice("data:image/png;base64,".length));
    };
    reader.readAsDataURL(blob);
  });
}

export function restoredPngBlob(base64: string): Blob {
  if (base64.length > Math.ceil(MAX_PHOTO_SOURCE_BYTES / 3) * 4 + 4) {
    throw new Error("Saved project PNG exceeds the memory limit.");
  }
  const binary = atob(base64);
  if (!binary.length || binary.length > MAX_PHOTO_SOURCE_BYTES) {
    throw new Error("Saved project PNG bytes are invalid or too large.");
  }
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return new Blob([bytes], { type: "image/png" });
}

async function resultOf<T>(work: () => Promise<T>): Promise<Result<T>> {
  try { return { ok: true, value: await work() }; }
  catch (error) { return { ok: false, error: errorText(error) }; }
}

export function projectPhotosClient(invoke: Invoke) {
  return {
    list: (projectId: string) =>
      resultOf(() => invoke<ProjectPhotoListing>("project_photo_list", { projectId })),
    open: (projectId: string, assetId: string) =>
      resultOf(() => invoke<OpenedProjectPhoto>("project_photo_open", { projectId, assetId })),
    save: (args: {
      projectId: string; assetId?: string; expectedRevision?: number;
      sourceName: string; sourceUrl: string; edits: PhotoAdjustments;
    }) => resultOf(async () => {
      const source = await sourcePngBase64(args.sourceUrl);
      return invoke<ProjectPhotoDocument>("project_photo_save", {
        request: {
          project_id: args.projectId,
          asset_id: args.assetId ?? null,
          expected_revision: args.expectedRevision ?? null,
          source_name: args.sourceName,
          source_png_base64: source,
          edits: recipeFromEdits(args.edits),
        },
      });
    }),
  };
}
