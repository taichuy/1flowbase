import {
  deleteConsoleInstalledExtension,
  listConsoleInstalledExtensions,
  selectConsoleInstalledExtension,
  type ConsoleInstalledExtension
} from '@1flowbase/api-client';

export type InstalledAgentFlowFamily = ConsoleInstalledExtension;

export const installedAgentFlowTemplatesQueryKey = [
  'templates',
  'installed-agent-flow'
] as const;

export function fetchInstalledAgentFlowTemplates() {
  return listConsoleInstalledExtensions(undefined, 50, 'agent-flow');
}

export function selectInstalledAgentFlowVersion(
  installationId: string,
  csrfToken: string
) {
  return selectConsoleInstalledExtension(installationId, csrfToken);
}

export function deleteInstalledAgentFlowVersion(
  installationId: string,
  csrfToken: string
) {
  return deleteConsoleInstalledExtension(installationId, csrfToken);
}
