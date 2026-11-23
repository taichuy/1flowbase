import type { ApplicationNamespace } from './application-i18n-resources';

// Strictly limit Layer-1 Topbar shell dictionaries to true chrome strings (~2.6 kB gz).
// Heavy backstage settings (62 kB), frontstage authoring (22 kB), and notification inbox
// (9 kB) dictionaries load only when their owning route, workspace, or drawer activates.
export const SHELL_TOPBAR_NAMESPACES: readonly ApplicationNamespace[] = [
  'app',
  'appShell',
  'shared',
  'sharedUi'
];

const SETTINGS_NAMESPACES: Record<string, readonly ApplicationNamespace[]> = {
  applications: ['settingsApplicationManagement'],
  backups: ['settingsSystemBackups', 'settingsSystemTemplates'],
  events: ['settingsEvents'],
  roles: ['settingsRecordPermissions'],
  'data-models': ['settingsRecordPermissions'],
  'mcp-management': ['settingsMcpManagement'],
  'ui-management': ['settingsUiManagement', 'settingsRecordPermissions'],
  'auth-center': ['settingsRecordPermissions'],
  'extension-center': [
    'settingsMcpManagement',
    'settingsBilling',
    'settingsSystemTemplates',
    'settingsUiManagement'
  ],
  billing: ['settingsBilling']
};

export function applicationTopbarNamespacesForPath(
  pathname: string
): readonly ApplicationNamespace[] {
  if (pathname === '/mcp/authorize') return [];
  const [, section] = pathname.split('/');
  if (
    section === 'me' ||
    section === 'settings' ||
    section === 'embedded-apps' ||
    section === 'applications'
  ) {
    return applicationNamespacesForPath(pathname);
  }
  return SHELL_TOPBAR_NAMESPACES;
}

export function applicationNamespacesForPath(
  pathname: string
): readonly ApplicationNamespace[] {
  if (pathname === '/mcp/authorize') return [];
  const [, section, detail] = pathname.split('/');
  const routeNamespaces: readonly ApplicationNamespace[] =
    section === 'me'
      ? ['me']
      : section === 'settings'
        ? ['settings', 'schemaUi', ...(SETTINGS_NAMESPACES[detail ?? ''] ?? [])]
        : section === 'embedded-apps'
          ? ['embeddedApps']
          : section === 'applications'
            ? ['applications', 'agentFlow', 'workflow', 'schemaUi']
            : ['frontstage', 'schemaUi'];
  return [...SHELL_TOPBAR_NAMESPACES, ...routeNamespaces];
}
