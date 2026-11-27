import { afterEach, describe, expect, test } from 'vitest';

import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../../shared/i18n/app-i18n';
import { createFrontstageNativeReactModuleRegistry } from '../registry';

const DYNAMIC_NAMESPACES = [
  'applications',
  'agentFlow',
  'workflow',
  'settingsRecordPermissions'
] as const;

function evictDynamicNamespaces() {
  for (const locale of ['zh_Hans', 'en_US'] as const) {
    for (const ns of DYNAMIC_NAMESPACES) {
      appI18n.removeResourceBundle(locale, ns);
    }
  }
}

describe('Frontstage native React module i18n namespace demand', () => {
  afterEach(async () => {
    await appI18n.changeLanguage('zh_Hans');
    await loadApplicationI18nResources();
  });

  test('loads applications, agentFlow, and workflow dictionaries before resolving @1flowbase/application-run-logs on cold frontstage start', async () => {
    await appI18n.changeLanguage('zh_Hans');
    evictDynamicNamespaces();

    expect(appI18n.hasResourceBundle('zh_Hans', 'applications')).toBe(false);
    expect(appI18n.t('auto.past_seven_days', { ns: 'applications' })).toBe(
      'auto.past_seven_days'
    );

    const registry = createFrontstageNativeReactModuleRegistry();
    const resolved = await registry.resolveInjectedModuleMap!([
      {
        source: '@1flowbase/application-run-logs',
        bindings: [
          {
            kind: 'named',
            source: '@1flowbase/application-run-logs',
            imported: 'AllAgentFlowLogs',
            local: 'AllAgentFlowLogs'
          }
        ]
      }
    ]);

    expect(resolved['@1flowbase/application-run-logs']).toHaveProperty(
      'AllAgentFlowLogs'
    );
    expect(appI18n.hasResourceBundle('zh_Hans', 'applications')).toBe(true);
    expect(appI18n.hasResourceBundle('zh_Hans', 'agentFlow')).toBe(true);
    expect(appI18n.hasResourceBundle('zh_Hans', 'workflow')).toBe(true);
    expect(appI18n.t('auto.past_seven_days', { ns: 'applications' })).toBe(
      '过去 7 天'
    );
    expect(appI18n.t('auto.sort_by_prefix', { ns: 'applications' })).toBe(
      '排序：'
    );
    expect(appI18n.t('auto.start_time', { ns: 'applications' })).toBe(
      '开始时间'
    );
    expect(appI18n.t('auto.search_title', { ns: 'applications' })).toBe(
      '搜索标题'
    );
  });

  test('loads settingsRecordPermissions dictionary only when RecordPermissionEditor is requested from @1flowbase/plugin-settings', async () => {
    await appI18n.changeLanguage('zh_Hans');
    evictDynamicNamespaces();

    const registry = createFrontstageNativeReactModuleRegistry();
    await registry.resolveInjectedModuleMap!([
      {
        source: '@1flowbase/plugin-settings',
        bindings: [
          {
            kind: 'named',
            source: '@1flowbase/plugin-settings',
            imported: 'request',
            local: 'request'
          }
        ]
      }
    ]);
    expect(
      appI18n.hasResourceBundle('zh_Hans', 'settingsRecordPermissions')
    ).toBe(false);

    const resolved = await registry.resolveInjectedModuleMap!([
      {
        source: '@1flowbase/plugin-settings',
        bindings: [
          {
            kind: 'named',
            source: '@1flowbase/plugin-settings',
            imported: 'RecordPermissionEditor',
            local: 'RecordPermissionEditor'
          }
        ]
      }
    ]);
    expect(resolved['@1flowbase/plugin-settings']).toHaveProperty(
      'RecordPermissionEditor'
    );
    expect(
      appI18n.hasResourceBundle('zh_Hans', 'settingsRecordPermissions')
    ).toBe(true);
  });
});
