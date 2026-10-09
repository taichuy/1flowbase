import { describe, expect, test } from 'vitest';
import { createBlockI18n } from '../translator';

describe('block catalog translation', () => {
  test('uses literal global keys, including dots, colons and English phrases', () => {
    const i18n = createBlockI18n({
      locale: 'zh_Hans',
      status: 'ready',
      messages: {
        Account: '账号',
        'a.b:c': '完整键',
        'Hello {{name}}': '你好 {{name}}'
      }
    });
    expect(i18n.t('Account')).toBe('账号');
    expect(i18n.t('a.b:c')).toBe('完整键');
    expect(i18n.t('Hello {{name}}', { values: { name: '<Ada>' } })).toBe(
      '你好 <Ada>'
    );
    expect(i18n.t('missing')).toBe('missing');
    expect(i18n.t('missing', { defaultValue: '默认文案' })).toBe('默认文案');
  });

  test('interpolation values cannot change the locale or translation options', () => {
    const i18n = createBlockI18n({
      locale: 'zh_Hans',
      status: 'ready',
      messages: {
        'Locale {{lng}}': '语言 {{lng}}'
      }
    });
    expect(
      i18n.t('Locale {{lng}}', {
        values: { lng: 'en_US', defaultValue: 'wrong' }
      })
    ).toBe('语言 en_US');
  });

  test('keeps snapshots isolated and exposes loading/error without old messages', () => {
    const zh = createBlockI18n({
      locale: 'zh_Hans',
      status: 'ready',
      messages: { Account: '账号' }
    });
    const en = createBlockI18n({
      locale: 'en_US',
      status: 'ready',
      messages: { Account: 'Account' }
    });
    expect(en.t('Account')).toBe('Account');
    expect(zh.t('Account')).toBe('账号');
    for (const status of ['loading', 'error'] as const) {
      const pending = createBlockI18n({ locale: 'en_US', status });
      expect(pending.status).toBe(status);
      expect(pending.t('Account')).toBe('Account');
    }
  });
});
