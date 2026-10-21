import { apiFetch } from '../transport';

export interface SystemMetadata {
  logo_url: string;
  system_name: string;
  login_title: string;
  system_description: string;
  filing_html: string;
}

export function fetchPublicSystemMetadata() {
  return apiFetch<SystemMetadata>({ path: '/api/public/system-metadata' });
}
