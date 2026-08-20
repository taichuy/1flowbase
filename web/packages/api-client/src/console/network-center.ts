import { apiFetch } from '../transport';

export interface ConsoleNetworkEgressProjection {
  provider_egress_key: string;
  display_name: string;
  region: string | null;
  tags: string[];
  availability: string;
  synced_at: string;
}

export interface ConsoleNetworkEgressProvider {
  id: string;
  installation_id: string;
  provider_code: string;
  display_name: string;
  lifecycle: string;
  health_status: string;
  secret_configured: boolean;
  last_sync_error: string | null;
  last_synced_at: string | null;
  egresses: ConsoleNetworkEgressProjection[];
}

export function listConsoleNetworkEgressProviders(baseUrl?: string) {
  return apiFetch<ConsoleNetworkEgressProvider[]>({
    path: '/api/console/settings/network-center/providers',
    baseUrl
  });
}
