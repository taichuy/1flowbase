export interface PublicSystemMetadataSeed {
  system_name?: string;
  logo_url?: string | null;
  hide_login_tagline?: boolean;
  system_description?: string | null;
  filing_html?: string | null;
  page_title?: string;
  login_hero_title?: string;
  login_hero_tagline?: string | null;
  hide_powered_by?: boolean;
  hide_help_menu?: boolean;
}

export const SYSTEM_METADATA_INVALIDATE_EVENT =
  '1flowbase:system-metadata-invalidate';

export function notifySystemMetadataChanged(): void {
  if (typeof window !== 'undefined') {
    window.dispatchEvent(new Event(SYSTEM_METADATA_INVALIDATE_EVENT));
  }
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
