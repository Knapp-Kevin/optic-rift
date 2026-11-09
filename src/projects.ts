// Client side of the native project metadata store (src-tauri/src/project_store.rs).
// Project metadata itself remains a small name/identity record. Optional managed PNG media and edit revisions live in the separately verified project_photos native store.

export type ProjectRecord = { schema_version: number; id: string; name: string; created_unix_ms: number };
export type ProjectSummary = { id: string; name: string; created_unix_ms: number };
export type ProjectProblem = { file_name: string; error: string };
export type ProjectListing = { projects: ProjectSummary[]; problems: ProjectProblem[] };

/** Minimal shape of Tauri's `invoke`, injectable so the client can be tested without a webview. */
export type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export const MAX_PROJECT_NAME_CHARS = 120;

/** Mirrors the native rule; the native store remains the authority and re-validates. */
export function projectNameError(input: string): string | null {
  const name = input.trim();
  if (name.length === 0) return "Enter a project name.";
  if (Array.from(name).length > MAX_PROJECT_NAME_CHARS) return `Use at most ${MAX_PROJECT_NAME_CHARS} characters.`;
  // Same set as Rust's char::is_control (Unicode Cc).
  if (/\p{Cc}/u.test(name)) return "Remove control characters such as line breaks or tabs.";
  return null;
}

/** Convert anything thrown by invoke into a user-readable message. */
export function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "The native project store did not respond. Projects are only available in the desktop app.";
}

export type Result<T> = { ok: true; value: T } | { ok: false; error: string };

async function call<T>(work: () => Promise<T>): Promise<Result<T>> {
  try {
    return { ok: true, value: await work() };
  } catch (error) {
    return { ok: false, error: errorText(error) };
  }
}

export function projectClient(invoke: Invoke) {
  return {
    list: () => call(() => invoke<ProjectListing>("project_list")),
    open: (id: string) => call(() => invoke<ProjectRecord>("project_open", { id })),
    async create(name: string): Promise<Result<ProjectRecord>> {
      const problem = projectNameError(name);
      if (problem) return { ok: false, error: problem };
      return call(() => invoke<ProjectRecord>("project_create", { name: name.trim() }));
    }
  };
}

export function formatCreated(unixMs: number): string {
  return new Date(unixMs).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}
