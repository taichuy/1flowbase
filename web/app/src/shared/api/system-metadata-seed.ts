export interface PublicSystemMetadataSeed {
  system_name?: string;
  logo_url?: string | null;
  hide_login_tagline?: boolean;
  system_description?: string | null;
  filing_html?: string | null;
}

let seededPublicSystemMetadata: PublicSystemMetadataSeed | undefined;
let seededAtTimestamp = 0;

export function setSeededPublicSystemMetadata(
  metadata: PublicSystemMetadataSeed | undefined
): void {
  if (!metadata) return;
  seededPublicSystemMetadata = {
    ...seededPublicSystemMetadata,
    ...metadata
  };
  seededAtTimestamp = Date.now();
}

export function getSeededPublicSystemMetadata():
  | PublicSystemMetadataSeed
  | undefined {
  return seededPublicSystemMetadata;
}

export function getSeededPublicSystemMetadataUpdatedAt(): number {
  return seededAtTimestamp;
}
