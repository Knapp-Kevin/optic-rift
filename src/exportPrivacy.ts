/** Explicit export consent: Pictures can be redirected to OneDrive or another sync provider.
 * The exported receipt has AI/model disclosures but deliberately omits private
 * prompts, runtime identifiers and source paths. This local JSON is UNSIGNED,
 * not a C2PA credential; the app-local source history remains unencrypted.
 * UI consent is not an OS-enforced cloud-sync prevention mechanism.
 */
export const PICTURES_EXPORT_WARNING =
  "Save this image and an unsigned AI disclosure to your Windows Pictures folder?\n\n" +
  "Pictures may be backed up by OneDrive or another cloud service. " +
  "Saving here may upload your image and the redacted disclosure (including model identities " +
  "and local generation IDs). Raw prompts are not copied into that disclosure, " +
  "but the original private generation history remains in Optic Rift’s local storage.\n\n" +
  "This JSON receipt is not signed, encrypted, or a C2PA Content Credential. " +
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
