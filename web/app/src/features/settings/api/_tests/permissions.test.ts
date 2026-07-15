import { describe, expect, test } from 'vitest';

import { settingsConsolePolicyCatalogQueryKey } from '../permissions';

describe('settings console policy catalog query', () => {
  test('partitions localized catalog results by canonical locale (Issue #1259 AC-009)', () => {
    expect(settingsConsolePolicyCatalogQueryKey('zh_Hans')).toEqual([
      'settings',
      'console-policy-catalog',
      'zh_Hans'
    ]);
    expect(settingsConsolePolicyCatalogQueryKey('en_US')).toEqual([
      'settings',
      'console-policy-catalog',
      'en_US'
    ]);
  });
});
