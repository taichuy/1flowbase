import { apiFetch } from '../transport/json';

export interface SystemBranding {
  logo_url: string;
  system_name: string;
  hide_login_tagline: boolean;
  system_description: string;
  filing_html: string;
}

export interface SystemMetadata extends SystemBranding {
  page_title?: string;
  login_hero_title?: string;
  login_hero_tagline?: string | null;
  hide_powered_by?: boolean;
  hide_help_menu?: boolean;
}

export function fetchPublicSystemMetadata() {
  return apiFetch<SystemMetadata>({
    path: '/api/public/system-metadata',
    credentials: 'omit'
  });
}
