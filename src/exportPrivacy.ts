/** Explicit export consent: Pictures can be redirected to OneDrive or another sync provider.
 * The saved provenance record includes the prompt, settings and source model identity.
 * This UI guard must never be treated as an OS-enforced cloud-sync prevention mechanism. */
export const PICTURES_EXPORT_WARNING =
  "Save this image and its provenance record to your Windows Pictures folder?\n\n" +
  "Pictures may be backed up by OneDrive or another cloud service. In that case, " +
  "saving here could upload your image and its generation details, including your prompt.\n\n" +
  "Choose Cancel to keep the result in Optic Rift’s local output folder.";

/** Do not start the native file copy until the user approves the destination disclosure. */
export async function saveWithPicturesConsent(
  jobId: string,
  confirm: (notice: string) => boolean,
  nativeSave: (jobId: string) => Promise<string>,
): Promise<string | null> {
  if (!confirm(PICTURES_EXPORT_WARNING)) return null;
  return nativeSave(jobId);
}
