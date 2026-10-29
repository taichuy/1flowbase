import { apiFetch } from '../transport';

export type HttpNodeNetworkEgress =
  | { mode: 'workspace' }
  | { mode: 'direct' }
  | { mode: 'proxy'; pool_member_id: string };

export interface HttpNodeProxyOption {
  pool_member_id: string;
  display_name: string;
  selectable: boolean;
}

export function fetchConsoleHttpNodeProxyOptions(baseUrl?: string) {
  return apiFetch<HttpNodeProxyOption[]>({
    path: '/api/console/network-center/http-node-proxy-options',
    baseUrl
  });
}
