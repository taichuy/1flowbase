import { apiFetch, apiFetchVoid } from '../transport';

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

export interface ConsoleNetworkEgressPoolMember {
  id: string;
  provider_id: string;
  provider_egress_key: string;
  enabled: boolean;
  sequence: number;
  health: string;
}

export interface ConsoleNetworkEgressPool {
  id: string;
  display_name: string;
  selection_strategy: string;
  members: ConsoleNetworkEgressPoolMember[];
}

export interface CreateConsoleNetworkEgressPoolInput {
  display_name: string;
}

export interface UpdateConsoleNetworkEgressPoolInput {
  display_name: string;
}

export interface CreateConsoleNetworkEgressPoolMemberInput {
  provider_id: string;
  provider_egress_key: string;
  enabled: boolean;
  sequence: number;
}

export interface UpdateConsoleNetworkEgressPoolMemberInput {
  enabled: boolean;
  sequence: number;
}

export function listConsoleNetworkEgressProviders(baseUrl?: string) {
  return apiFetch<ConsoleNetworkEgressProvider[]>({
    path: '/api/console/settings/network-center/providers',
    baseUrl
  });
}

export function listConsoleNetworkEgressPools(baseUrl?: string) {
  return apiFetch<ConsoleNetworkEgressPool[]>({
    path: '/api/console/network-center/pools',
    baseUrl
  });
}

export function createConsoleNetworkEgressPool(
  input: CreateConsoleNetworkEgressPoolInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressPool>({
    path: '/api/console/network-center/pools',
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleNetworkEgressPool(
  poolId: string,
  input: UpdateConsoleNetworkEgressPoolInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressPool>({
    path: `/api/console/network-center/pools/${encodeURIComponent(poolId)}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleNetworkEgressPool(
  poolId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/network-center/pools/${encodeURIComponent(poolId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}

export function createConsoleNetworkEgressPoolMember(
  poolId: string,
  input: CreateConsoleNetworkEgressPoolMemberInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressPoolMember>({
    path: `/api/console/network-center/pools/${encodeURIComponent(poolId)}/members`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function updateConsoleNetworkEgressPoolMember(
  poolId: string,
  memberId: string,
  input: UpdateConsoleNetworkEgressPoolMemberInput,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetch<ConsoleNetworkEgressPoolMember>({
    path: `/api/console/network-center/pools/${encodeURIComponent(poolId)}/members/${encodeURIComponent(memberId)}`,
    method: 'PATCH',
    body: input,
    csrfToken,
    baseUrl
  });
}

export function deleteConsoleNetworkEgressPoolMember(
  poolId: string,
  memberId: string,
  csrfToken: string,
  baseUrl?: string
) {
  return apiFetchVoid({
    path: `/api/console/network-center/pools/${encodeURIComponent(poolId)}/members/${encodeURIComponent(memberId)}`,
    method: 'DELETE',
    csrfToken,
    baseUrl
  });
}
