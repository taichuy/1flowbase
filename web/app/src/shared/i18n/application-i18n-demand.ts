import type { ApplicationNamespace } from './application-i18n-resources';

// Include the shell's immediately available actions and retained assistant/window
// surfaces, not only the current route's center panel. These namespaces are shared
// with those consumers; route-only settings dictionaries remain independently lazy.
const SHELL_NAMESPACES: readonly ApplicationNamespace[] = [
  'app',
  'appShell',
  'shared',
  'sharedUi',
  'frontstage',
  'settings', // SettingsChromeMenu renders backend catalog labels immediately.
  'notifications',
  'agentFlow', // EmbeddedAgentAssistant / AgentFlowDockPanel and retained chat.
  'workflow', // Assistant selector-options labels.
  'schemaUi', // Shared dock/overlay surfaces used by shell actions.
  'applications' // Assistant ConversationLogPanel and related log actions.
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

export function applicationNamespacesForPath(
  pathname: string
): readonly ApplicationNamespace[] {
  if (pathname === '/mcp/authorize') return [];
  // Auth may settle while the browser still displays /sign-in. The authenticated
  // router redirects it to /, so prepare that shell before any runtime is shown.
  // Public sign-in never calls this authenticated demand resolver.
  const [, section, detail] = pathname.split('/');
  const routeNamespaces: readonly ApplicationNamespace[] =
    section === 'me'
      ? ['me']
      : section === 'settings'
        ? (SETTINGS_NAMESPACES[detail ?? ''] ?? [])
        : section === 'embedded-apps'
          ? ['embeddedApps']
          : section === 'applications'
            ? []
            : ['settingsRecordPermissions'];
  return [...SHELL_NAMESPACES, ...routeNamespaces];
}
