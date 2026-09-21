import {
  clearConsoleHostInfrastructureCacheDomain,
  clearConsoleHostInfrastructureCacheEntry,
  getConsoleHostInfrastructureMemoryOverview,
  getConsoleHostInfrastructureMemoryStatsOverview,
  getConsoleHostInfrastructureMemoryStats,
  getConsoleHostInfrastructureCacheOverview,
  listConsoleHostInfrastructureCacheEntries,
  listConsoleHostInfrastructureMemoryEntries,
  listConsoleHostInfrastructureMemoryTree,
  revealConsoleHostInfrastructureCacheEntry,
  revealConsoleHostInfrastructureMemoryEntry,
  searchConsoleHostInfrastructureMemoryEntries,
  type ConsoleCacheEntryMetadata,
  type ConsoleCacheEntryValue,
  type ConsoleCacheDomain,
  type ConsoleHostInfrastructureMemoryEntries,
  type ConsoleHostInfrastructureMemoryOverview,
  type ConsoleMemoryStatsOverview,
  type ConsoleMemoryStats,
  type ConsoleHostInfrastructureCacheEntries,
  type ConsoleHostInfrastructureCacheOverview,
  type ConsoleMemoryContractSummary,
  type ConsoleMemoryEntryMetadata,
  type ConsoleMemoryEntryValue,
  type ConsoleMemoryPageRequest,
  type ConsoleMemoryRevealMode,
  type ConsoleMemorySearchRequest,
  type ConsoleMemoryTreeNode,
  type ConsoleHostInfrastructureMemoryTree,
} from '@1flowbase/api-client';

export type SettingsHostInfrastructureCacheOverview =
  ConsoleHostInfrastructureCacheOverview;
export type SettingsHostInfrastructureCacheEntries =
  ConsoleHostInfrastructureCacheEntries;
export type SettingsHostInfrastructureCacheDomain = ConsoleCacheDomain;
export type SettingsHostInfrastructureCacheEntry = ConsoleCacheEntryMetadata;
export type SettingsHostInfrastructureCacheEntryValue = ConsoleCacheEntryValue;
export type SettingsHostInfrastructureMemoryOverview =
  ConsoleHostInfrastructureMemoryOverview;
export type SettingsHostInfrastructureMemoryEntries =
  ConsoleHostInfrastructureMemoryEntries;
export type SettingsHostInfrastructureMemoryContract =
  ConsoleMemoryContractSummary;
export type SettingsHostInfrastructureMemoryStatsOverview =
  ConsoleMemoryStatsOverview;
export type SettingsHostInfrastructureMemoryStats = ConsoleMemoryStats;
export type SettingsHostInfrastructureMemoryEntry = ConsoleMemoryEntryMetadata;
export type SettingsHostInfrastructureMemoryEntryValue =
  ConsoleMemoryEntryValue;
export type SettingsHostInfrastructureMemoryTree =
  ConsoleHostInfrastructureMemoryTree;
export type SettingsHostInfrastructureMemoryTreeNode = ConsoleMemoryTreeNode;
export type SettingsHostInfrastructureMemoryPageRequest =
  ConsoleMemoryPageRequest;
export type SettingsHostInfrastructureMemorySearchRequest =
  ConsoleMemorySearchRequest;
export type SettingsHostInfrastructureMemoryRevealMode =
  ConsoleMemoryRevealMode;

export const settingsHostInfrastructureCacheOverviewQueryKey = [
  'settings',
  'host-infrastructure',
  'cache'
] as const;

export const settingsHostInfrastructureMemoryOverviewQueryKey = [
  'settings',
  'host-infrastructure',
  'memory'
] as const;

export const settingsHostInfrastructureMemoryStatsOverviewQueryKey = [
  'settings',
  'host-infrastructure',
  'memory',
  'stats'
] as const;

export function settingsHostInfrastructureCacheEntriesQueryKey(
  domainCode: string | null
) {
  return [
    'settings',
    'host-infrastructure',
    'cache',
    'domains',
    domainCode,
    'entries'
  ] as const;
}

export function settingsHostInfrastructureMemoryEntriesQueryKey(
  contractCode: string | null,
  request?: SettingsHostInfrastructureMemoryPageRequest
) {
  return [
    'settings',
    'host-infrastructure',
    'memory',
    'contracts',
    contractCode,
    'entries',
    request?.inspection_path ?? [],
    request?.cursor ?? null,
    request?.limit ?? null,
    request?.byte_limit ?? null
  ] as const;
}

export function settingsHostInfrastructureMemoryTreeQueryKey(
  contractCode: string | null,
  request?: SettingsHostInfrastructureMemoryPageRequest
) {
  return [
    'settings',
    'host-infrastructure',
    'memory',
    'contracts',
    contractCode,
    'tree',
    request?.inspection_path ?? [],
    request?.cursor ?? null,
    request?.limit ?? null,
    request?.byte_limit ?? null
  ] as const;
}

export function settingsHostInfrastructureMemoryStatsQueryKey(
  contractCode: string | null,
  request?: Pick<SettingsHostInfrastructureMemoryPageRequest, 'inspection_path'>
) {
  return [
    'settings',
    'host-infrastructure',
    'memory',
    'contracts',
    contractCode,
    'stats',
    request?.inspection_path ?? []
  ] as const;
}

export function settingsHostInfrastructureMemorySearchQueryKey(
  contractCode: string | null,
  request?: SettingsHostInfrastructureMemorySearchRequest
) {
  return [
    'settings',
    'host-infrastructure',
    'memory',
    'contracts',
    contractCode,
    'search',
    request?.q ?? '',
    request?.inspection_path ?? [],
    request?.cursor ?? null,
    request?.limit ?? null,
    request?.byte_limit ?? null
  ] as const;
}

export function fetchSettingsHostInfrastructureCacheOverview() {
  return getConsoleHostInfrastructureCacheOverview();
}

export function fetchSettingsHostInfrastructureMemoryOverview() {
  return getConsoleHostInfrastructureMemoryOverview();
}

export function fetchSettingsHostInfrastructureMemoryStatsOverview() {
  return getConsoleHostInfrastructureMemoryStatsOverview();
}

export function fetchSettingsHostInfrastructureMemoryStats(
  contractCode: string,
  request?: Pick<SettingsHostInfrastructureMemoryPageRequest, 'inspection_path'>
) {
  return getConsoleHostInfrastructureMemoryStats(contractCode, request);
}

export function fetchSettingsHostInfrastructureCacheEntries(
  domainCode: string
) {
  return listConsoleHostInfrastructureCacheEntries(domainCode);
}

export function fetchSettingsHostInfrastructureMemoryEntries(
  contractCode: string,
  request?: SettingsHostInfrastructureMemoryPageRequest
) {
  return listConsoleHostInfrastructureMemoryEntries(contractCode, request);
}

export function fetchSettingsHostInfrastructureMemoryTree(
  contractCode: string,
  request?: SettingsHostInfrastructureMemoryPageRequest
) {
  return listConsoleHostInfrastructureMemoryTree(contractCode, request);
}

export function searchSettingsHostInfrastructureMemoryEntries(
  contractCode: string,
  request: SettingsHostInfrastructureMemorySearchRequest
) {
  return searchConsoleHostInfrastructureMemoryEntries(contractCode, request);
}

export function revealSettingsHostInfrastructureCacheEntry(
  domainCode: string,
  key: string,
  csrfToken: string
) {
  return revealConsoleHostInfrastructureCacheEntry(domainCode, key, csrfToken);
}

export function revealSettingsHostInfrastructureMemoryEntry(
  contractCode: string,
  entryRef: string,
  csrfToken: string,
  revealMode: SettingsHostInfrastructureMemoryRevealMode = 'preview'
) {
  return revealConsoleHostInfrastructureMemoryEntry(
    contractCode,
    entryRef,
    csrfToken,
    revealMode
  );
}

export function clearSettingsHostInfrastructureCacheEntry(
  domainCode: string,
  key: string,
  csrfToken: string
) {
  return clearConsoleHostInfrastructureCacheEntry(domainCode, key, csrfToken);
}

export function clearSettingsHostInfrastructureCacheDomain(
  domainCode: string,
  csrfToken: string
) {
  return clearConsoleHostInfrastructureCacheDomain(domainCode, csrfToken);
}
