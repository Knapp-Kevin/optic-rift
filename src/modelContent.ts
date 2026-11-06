/** Content labels describe a model's documented intended audience, not output legality or runtime support. */
export type ContentAudience = "general" | "mature" | "adult-focused" | "unverified";
export type ContentEvidence = "reviewed-model-card" | "developer-review" | "user-reported" | "unknown";

export interface ModelContentDescriptor {
  audience: ContentAudience;
  evidence: ContentEvidence;
}

/** For unknown or unreviewed provenance, never imply the model is known-safe or general purpose. */
export function displayAudience(descriptor: ModelContentDescriptor): string {
  if (descriptor.evidence === "unknown" || descriptor.audience === "unverified") return "Content classification unverified";
  switch (descriptor.audience) {
    case "adult-focused": return "Adult-focused (18+) · Explicit";
    case "mature": return "Mature themes";
    case "general": return "General purpose";
  }
}

/** Discovery filtering affects listings, never the user's locally imported files or projects. */
export function listInDiscovery(descriptor: ModelContentDescriptor, includeAdultModels: boolean): boolean {
  return !(descriptor.audience === "adult-focused" && !includeAdultModels);
}

/** No explicit/unverified content automatically appears as a discovery preview, even after opt-in. */
export function allowAutomaticPreview(descriptor: ModelContentDescriptor): boolean {
  return descriptor.audience === "general" && (descriptor.evidence === "reviewed-model-card" || descriptor.evidence === "developer-review");
}
