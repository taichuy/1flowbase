// Each import is a locale × namespace boundary. Do not replace this with an
// eager JSON barrel: the public shell must not download unrelated dictionaries.
export const applicationTranslationLoaders = {
  agentFlow: {
    zh_Hans: () =>
      import('../../features/agent-flow/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/agent-flow/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  app: {
    zh_Hans: () =>
      import('../../app/i18n/zh_Hans.json').then((module) => module.default),
    en_US: () =>
      import('../../app/i18n/en_US.json').then((module) => module.default)
  },
  appShell: {
    zh_Hans: () =>
      import('../../app-shell/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../app-shell/i18n/en_US.json').then((module) => module.default)
  },
  applications: {
    zh_Hans: () =>
      import('../../features/applications/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/applications/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  embeddedApps: {
    zh_Hans: () =>
      import('../../features/embedded-apps/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/embedded-apps/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  frontstage: {
    zh_Hans: () =>
      import('../../features/frontstage/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/frontstage/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  me: {
    zh_Hans: () =>
      import('../../features/me/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/me/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  notifications: {
    zh_Hans: () =>
      import('../../features/notifications/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/notifications/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  schemaUi: {
    zh_Hans: () =>
      import('../../shared/schema-ui/v1/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../shared/schema-ui/v1/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  settingsApplicationManagement: {
    zh_Hans: () =>
      import('../../features/settings/components/application-management/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/settings/components/application-management/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  settingsSystemBackups: {
    zh_Hans: () =>
      import('../../features/settings/components/system-backups/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/settings/components/system-backups/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  settingsSystemTemplates: {
    zh_Hans: () =>
      import('../../features/settings/components/system-templates/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/settings/components/system-templates/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  settingsMcpManagement: {
    zh_Hans: () =>
      import('../../features/settings/components/mcp-management/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/settings/components/mcp-management/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  settingsUiManagement: {
    zh_Hans: () =>
      import('../../features/settings/components/ui-management/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/settings/components/ui-management/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  settingsBilling: {
    zh_Hans: () =>
      import('../../features/settings/components/billing/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/settings/components/billing/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  settingsEvents: {
    zh_Hans: () =>
      import('../../features/settings/pages/events/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/settings/pages/events/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  settingsRecordPermissions: {
    zh_Hans: () =>
      import('../../features/settings/components/record-permissions/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/settings/components/record-permissions/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  settings: {
    zh_Hans: () =>
      import('../../features/settings/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/settings/i18n/en_US.json').then(
        (module) => module.default
      )
  },
  shared: {
    zh_Hans: () =>
      import('./resources/zh_Hans.json').then((module) => module.default),
    en_US: () =>
      import('./resources/en_US.json').then((module) => module.default)
  },
  sharedUi: {
    zh_Hans: () =>
      import('../../shared/ui/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../shared/ui/i18n/en_US.json').then((module) => module.default)
  },
  workflow: {
    zh_Hans: () =>
      import('../../features/workflow/i18n/zh_Hans.json').then(
        (module) => module.default
      ),
    en_US: () =>
      import('../../features/workflow/i18n/en_US.json').then(
        (module) => module.default
      )
  }
} as const;

export type ApplicationNamespace = keyof typeof applicationTranslationLoaders;
