import i18next from 'i18next';
import { expect, test, vi } from 'vitest';

import { createApplicationResourceLoader } from '../application-resource-loader';
import { applicationTranslationLoaders } from '../application-i18n-resources';
import {
  SHELL_TOPBAR_NAMESPACES,
  applicationNamespacesForPath,
  applicationTopbarNamespacesForPath
} from '../application-i18n-demand';
import { FALLBACK_APP_LOCALE } from '../locales';

function coldInstance() {
  // Do NOT import appI18n: global setup loads every dictionary into that instance.
  const instance = i18next.createInstance();
  void instance.init({
    resources: {},
    lng: 'en_US',
    fallbackLng: FALLBACK_APP_LOCALE,
    initAsync: false
  });
  return instance;
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

test('a truly cold English frontstage load excludes Chinese and route-only settings dictionaries', async () => {
  const instance = coldInstance();
  const loader = createApplicationResourceLoader(
    instance,
    applicationTranslationLoaders
  );
  expect(Object.keys(instance.store.data)).toEqual([]);
  await loader.activate(
    'en_US',
    applicationNamespacesForPath('/workspace/page/page-a')
  );
  expect(Object.keys(instance.store.data)).toEqual(['en_US']);
  expect(instance.hasResourceBundle('en_US', 'frontstage')).toBe(true);
  for (const namespace of [
    'settingsSystemBackups',
    'settingsBilling',
    'settingsUiManagement',
    'me'
  ]) {
    expect(instance.hasResourceBundle('en_US', namespace)).toBe(false);
  }
  expect(instance.t('auto.unnamed_page', { ns: 'frontstage' })).not.toBe(
    'auto.unnamed_page'
  );
  await loader.activate(
    'en_US',
    applicationNamespacesForPath('/settings/backups')
  );
  expect(instance.hasResourceBundle('en_US', 'settingsSystemBackups')).toBe(
    true
  );
  expect(instance.hasResourceBundle('en_US', 'settingsSystemTemplates')).toBe(
    true
  );
  expect(instance.hasResourceBundle('en_US', 'settingsBilling')).toBe(false);
});

test('loads only requested locale/namespace and fallback, deduplicates cold flights and reuses warm bundles', async () => {
  const instance = coldInstance();
  const chinese = deferred<object>();
  const loaders = {
    page: {
      zh_Hans: vi.fn(() => chinese.promise),
      en_US: vi.fn(async () => ({
        greeting: 'Hello',
        fallback: 'English fallback'
      }))
    },
    unrelated: {
      zh_Hans: vi.fn(async () => ({})),
      en_US: vi.fn(async () => ({}))
    }
  };
  const loader = createApplicationResourceLoader(instance, loaders);
  const first = loader.load('zh_Hans', ['page']);
  const second = loader.load('zh_Hans', ['page']);
  expect(loaders.page.zh_Hans).toHaveBeenCalledTimes(1);
  expect(loaders.page.en_US).toHaveBeenCalledTimes(1);
  expect(loaders.unrelated.en_US).not.toHaveBeenCalled();
  chinese.resolve({ greeting: '你好' });
  await Promise.all([first, second]);
  await loader.activate('zh_Hans', ['page']);
  expect(instance.t('greeting', { ns: 'page' })).toBe('你好');
  expect(instance.t('fallback', { ns: 'page' })).toBe('English fallback');
  await loader.activate('en_US', ['page']);
  expect(instance.t('greeting', { ns: 'page' })).toBe('Hello');
  expect(loaders.page.en_US).toHaveBeenCalledTimes(1);
});

test('a language switch keeps the applied language and plugin catalog until complete; latest request wins', async () => {
  const instance = coldInstance();
  const chinese = deferred<object>();
  instance.addResourceBundle('en_US', 'plugin:sample', {
    action: 'Plugin action'
  });
  const loader = createApplicationResourceLoader(instance, {
    page: {
      en_US: async () => ({ greeting: 'Hello' }),
      zh_Hans: () => chinese.promise
    }
  });
  await loader.activate('en_US', ['page']);
  const pending = loader.activate('zh_Hans');
  expect(instance.language).toBe('en_US');
  expect(instance.t('greeting', { ns: 'page' })).toBe('Hello');
  await loader.activate('en_US');
  chinese.resolve({ greeting: '你好' });
  expect(await pending).toBe(false);
  expect(instance.language).toBe('en_US');
  expect(instance.t('action', { ns: 'plugin:sample' })).toBe('Plugin action');
  await loader.activate('zh_Hans');
  expect(instance.t('greeting', { ns: 'page' })).toBe('你好');
});

test('warm route preparation does not broadcast another language change', async () => {
  const instance = coldInstance();
  const loader = createApplicationResourceLoader(instance, {
    page: {
      en_US: async () => ({ title: 'Page' }),
      zh_Hans: async () => ({ title: '页面' })
    }
  });
  await loader.activate('en_US', ['page']);
  const changed = vi.fn();
  instance.on('languageChanged', changed);
  await loader.activate('en_US', ['page']);
  expect(changed).not.toHaveBeenCalled();
  await loader.activate('zh_Hans', ['page']);
  expect(changed).toHaveBeenCalledTimes(1);
});

test('failed imports preserve the old language and are retryable', async () => {
  const instance = coldInstance();
  const loadChinese = vi
    .fn()
    .mockRejectedValueOnce(new Error('offline'))
    .mockResolvedValue({ greeting: '你好' });
  const loader = createApplicationResourceLoader(instance, {
    page: { en_US: async () => ({ greeting: 'Hello' }), zh_Hans: loadChinese }
  });
  await loader.activate('en_US', ['page']);
  await expect(loader.activate('zh_Hans')).rejects.toThrow('offline');
  expect(instance.language).toBe('en_US');
  await loader.activate('zh_Hans');
  expect(instance.t('greeting', { ns: 'page' })).toBe('你好');
  expect(loadChinese).toHaveBeenCalledTimes(2);
});

test('route namespaces requested during a locale switch are ready before it commits', async () => {
  const instance = coldInstance();
  const pageChinese = deferred<object>();
  const settingsChinese = deferred<object>();
  const loader = createApplicationResourceLoader(instance, {
    page: {
      en_US: async () => ({ title: 'Page' }),
      zh_Hans: () => pageChinese.promise
    },
    settings: {
      en_US: async () => ({ title: 'Settings' }),
      zh_Hans: () => settingsChinese.promise
    }
  });
  await loader.activate('en_US', ['page']);
  const switching = loader.activate('zh_Hans');
  await loader.load('en_US', ['settings']);
  pageChinese.resolve({ title: '页面' });
  await Promise.resolve();
  expect(instance.language).toBe('en_US');
  settingsChinese.resolve({ title: '设置' });
  await switching;
  expect(instance.t('title', { ns: 'settings' })).toBe('设置');
});

test('authenticated /sign-in prepares the home shell before redirect, not an empty namespace set', async () => {
  const instance = coldInstance();
  const loader = createApplicationResourceLoader(
    instance,
    applicationTranslationLoaders
  );
  const demand = applicationNamespacesForPath('/sign-in');
  expect(demand).toEqual(applicationNamespacesForPath('/'));
  expect(demand).toContain('appShell');
  expect(demand).toContain('frontstage');
  await loader.activate('zh_Hans', demand);
  expect(instance.t('auto.unnamed_page', { ns: 'frontstage' })).not.toBe(
    'auto.unnamed_page'
  );
  expect(loader.ready('zh_Hans', applicationNamespacesForPath('/'))).toBe(true);
  expect(instance.hasResourceBundle('zh_Hans', 'settingsSystemBackups')).toBe(
    false
  );
});

test('activatePrimary resolves active locale immediately without blocking on fallback locale', async () => {
  const instance = coldInstance();
  const english = deferred<object>();
  const scheduled: Array<() => void> = [];
  const loaders = {
    page: {
      zh_Hans: vi.fn(async () => ({ greeting: '你好' })),
      en_US: vi.fn(() => english.promise)
    }
  };
  const loader = createApplicationResourceLoader(instance, loaders, {
    scheduleBackground: (task) => {
      scheduled.push(task);
    }
  });

  expect(loader.readyPrimary('zh_Hans', ['page'])).toBe(false);
  await loader.activatePrimary('zh_Hans', ['page']);

  expect(loaders.page.zh_Hans).toHaveBeenCalledTimes(1);
  expect(loaders.page.en_US).not.toHaveBeenCalled();
  expect(loader.readyPrimary('zh_Hans', ['page'])).toBe(true);
  expect(instance.language).toBe('zh_Hans');
  expect(instance.t('greeting', { ns: 'page' })).toBe('你好');
  expect(scheduled).toHaveLength(1);

  scheduled[0]!();
  expect(loaders.page.en_US).toHaveBeenCalledTimes(1);
  english.resolve({ greeting: 'Hello', fallback: 'Fallback text' });
  await waitForBundle(english.promise);
  expect(instance.t('fallback', { ns: 'page' })).toBe('Fallback text');
});

test('default activatePrimary avoids eager fallback bundle downloads and loads fallback only on missingKey', async () => {
  const instance = coldInstance();
  const loaders = {
    page: {
      zh_Hans: vi.fn(async () => ({ greeting: '你好' })),
      en_US: vi.fn(async () => ({ greeting: 'Hello', onlyInEn: 'Fallback value' }))
    }
  };
  const loader = createApplicationResourceLoader(instance, loaders);

  await loader.activatePrimary('zh_Hans', ['page']);
  expect(loaders.page.zh_Hans).toHaveBeenCalledTimes(1);
  expect(loaders.page.en_US).not.toHaveBeenCalled();
  expect(instance.t('greeting', { ns: 'page' })).toBe('你好');
  expect(loaders.page.en_US).not.toHaveBeenCalled();

  instance.emit('missingKey', ['zh_Hans'], 'page', 'onlyInEn', 'onlyInEn');
  await waitForBundle(Promise.resolve());
  expect(loaders.page.en_US).toHaveBeenCalledTimes(1);
});

test('keeps root topbar namespace demand strictly scoped to SHELL_TOPBAR_NAMESPACES across all authenticated routes', () => {
  for (const pathname of [
    '/',
    '/sales/pages/page-a',
    '/settings/backups',
    '/settings/extension-center/installed',
    '/applications/app-1/orchestration',
    '/me/security'
  ]) {
    expect(applicationTopbarNamespacesForPath(pathname)).toEqual(
      SHELL_TOPBAR_NAMESPACES
    );
  }
  expect(applicationTopbarNamespacesForPath('/mcp/authorize')).toEqual([]);
  expect(applicationNamespacesForPath('/settings/backups')).toEqual(
    expect.arrayContaining(['settings', 'settingsSystemBackups'])
  );
  expect(applicationNamespacesForPath('/applications/app-1/orchestration')).toEqual(
    expect.arrayContaining(['applications', 'agentFlow', 'workflow'])
  );
});

test('demandPrimary loads only active locale for dynamically mounted features and retains them across language switches', async () => {
  const instance = coldInstance();
  const loader = createApplicationResourceLoader(
    instance,
    applicationTranslationLoaders
  );
  await loader.activatePrimary(
    'zh_Hans',
    applicationNamespacesForPath('/route')
  );
  expect(instance.hasResourceBundle('zh_Hans', 'applications')).toBe(false);
  expect(instance.t('auto.past_seven_days', { ns: 'applications' })).toBe(
    'auto.past_seven_days'
  );

  await loader.demandPrimary('zh_Hans', [
    'applications',
    'agentFlow',
    'workflow'
  ]);
  expect(instance.hasResourceBundle('zh_Hans', 'applications')).toBe(true);
  expect(instance.hasResourceBundle('en_US', 'applications')).toBe(false);
  expect(instance.t('auto.past_seven_days', { ns: 'applications' })).toBe(
    '过去 7 天'
  );
  expect(instance.t('auto.sort_by_prefix', { ns: 'applications' })).toBe(
    '排序：'
  );

  await loader.activatePrimary('en_US');
  expect(instance.language).toBe('en_US');
  expect(instance.hasResourceBundle('en_US', 'applications')).toBe(true);
  expect(instance.hasResourceBundle('en_US', 'agentFlow')).toBe(true);
  expect(instance.t('auto.past_seven_days', { ns: 'applications' })).toBe(
    'past 7 days'
  );
});

async function waitForBundle(promise: Promise<unknown>) {
  await promise;
  await Promise.resolve();
}
