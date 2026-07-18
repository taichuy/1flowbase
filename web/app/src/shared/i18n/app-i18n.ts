import i18next, { type Resource } from 'i18next';
import { initReactI18next } from 'react-i18next';

import {
  FALLBACK_APP_LOCALE,
  resolveAppLocale,
  SUPPORTED_APP_LOCALES
} from './locales';
import {
  readLocalePreferenceFromStorage,
  readLocalePreferenceFromUrl
} from '../user-preferences/locale-preference';

import agentFlowZhHans from '../../features/agent-flow/i18n/zh_Hans.json';
import agentFlowEnUS from '../../features/agent-flow/i18n/en_US.json';
import appZhHans from '../../app/i18n/zh_Hans.json';
import appEnUS from '../../app/i18n/en_US.json';
import appShellZhHans from '../../app-shell/i18n/zh_Hans.json';
import appShellEnUS from '../../app-shell/i18n/en_US.json';
import applicationsZhHans from '../../features/applications/i18n/zh_Hans.json';
import applicationsEnUS from '../../features/applications/i18n/en_US.json';
import authZhHans from '../../features/auth/i18n/zh_Hans.json';
import authEnUS from '../../features/auth/i18n/en_US.json';
import embeddedAppsZhHans from '../../features/embedded-apps/i18n/zh_Hans.json';
import embeddedAppsEnUS from '../../features/embedded-apps/i18n/en_US.json';
import frontstageZhHans from '../../features/frontstage/i18n/zh_Hans.json';
import frontstageEnUS from '../../features/frontstage/i18n/en_US.json';
import meZhHans from '../../features/me/i18n/zh_Hans.json';
import meEnUS from '../../features/me/i18n/en_US.json';
import schemaUiZhHans from '../../shared/schema-ui/v1/i18n/zh_Hans.json';
import schemaUiEnUS from '../../shared/schema-ui/v1/i18n/en_US.json';
import settingsApplicationManagementZhHans from '../../features/settings/components/application-management/i18n/zh_Hans.json';
import settingsApplicationManagementEnUS from '../../features/settings/components/application-management/i18n/en_US.json';
import settingsMcpManagementZhHans from '../../features/settings/components/mcp-management/i18n/zh_Hans.json';
import settingsMcpManagementEnUS from '../../features/settings/components/mcp-management/i18n/en_US.json';
import settingsZhHans from '../../features/settings/i18n/zh_Hans.json';
import settingsEnUS from '../../features/settings/i18n/en_US.json';
import sharedZhHans from './resources/zh_Hans.json';
import sharedEnUS from './resources/en_US.json';
import sharedUiZhHans from '../../shared/ui/i18n/zh_Hans.json';
import sharedUiEnUS from '../../shared/ui/i18n/en_US.json';
import templatesZhHans from '../../features/templates/i18n/zh_Hans.json';
import workflowZhHans from '../../features/workflow/i18n/zh_Hans.json';
import workflowEnUS from '../../features/workflow/i18n/en_US.json';
import templatesEnUS from '../../features/templates/i18n/en_US.json';

const appTranslationResources = {
  zh_Hans: {
    agentFlow: agentFlowZhHans,
    app: appZhHans,
    appShell: appShellZhHans,
    applications: applicationsZhHans,
    auth: authZhHans,
    embeddedApps: embeddedAppsZhHans,
    frontstage: frontstageZhHans,
    me: meZhHans,
    schemaUi: schemaUiZhHans,
    settingsApplicationManagement: settingsApplicationManagementZhHans,
    settingsMcpManagement: settingsMcpManagementZhHans,
    settings: settingsZhHans,
    shared: sharedZhHans,
    sharedUi: sharedUiZhHans,
    templates: templatesZhHans,
    workflow: workflowZhHans
  },
  en_US: {
    agentFlow: agentFlowEnUS,
    app: appEnUS,
    appShell: appShellEnUS,
    applications: applicationsEnUS,
    auth: authEnUS,
    embeddedApps: embeddedAppsEnUS,
    frontstage: frontstageEnUS,
    me: meEnUS,
    schemaUi: schemaUiEnUS,
    settingsApplicationManagement: settingsApplicationManagementEnUS,
    settingsMcpManagement: settingsMcpManagementEnUS,
    settings: settingsEnUS,
    shared: sharedEnUS,
    sharedUi: sharedUiEnUS,
    templates: templatesEnUS,
    workflow: workflowEnUS
  }
} as const;

type AppTranslationNamespace =
  keyof (typeof appTranslationResources)[typeof FALLBACK_APP_LOCALE];

const appTranslationNamespaces = Object.keys(
  appTranslationResources[FALLBACK_APP_LOCALE]
) as AppTranslationNamespace[];

function getInitialAppLocale() {
  return resolveAppLocale(
    readLocalePreferenceFromStorage() ?? readLocalePreferenceFromUrl()
  );
}

export const appI18n = i18next.createInstance();

void appI18n
  .use(initReactI18next)
  .init({
    resources: appTranslationResources as unknown as Resource,
    lng: getInitialAppLocale(),
    fallbackLng: FALLBACK_APP_LOCALE,
    ns: appTranslationNamespaces,
    supportedLngs: [...SUPPORTED_APP_LOCALES],
    defaultNS: 'me',
    initAsync: false,
    interpolation: {
      escapeValue: false
    },
    react: {
      useSuspense: false
    }
  });
