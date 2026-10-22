import { apiFetch } from '../transport';

export interface SystemMetadata {
  logo_url: string;
  system_name: string;
  hide_login_tagline: boolean;
  system_description: string;
  filing_html: string;
}

export function fetchPublicSystemMetadata() {
  return apiFetch<SystemMetadata>({ path: '/api/public/system-metadata' });
}
