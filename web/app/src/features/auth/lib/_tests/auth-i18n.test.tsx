import { act, render, screen } from '@testing-library/react';
import { beforeEach, expect, test } from 'vitest';
import en from '../../i18n/en_US.json';
import zh from '../../i18n/zh_Hans.json';
import { authText, useAuthTranslation } from '../auth-i18n';
import { writeLocalePreferenceToStorage } from '../../../../shared/user-preferences/locale-preference';

beforeEach(() => {
  localStorage.clear();
  history.replaceState({}, '', '/sign-in');
});
test('cold locale uses URL, storage precedence, aliases and English fallback without appI18n', () => {
  expect(authText('sign_in.fallback_title')).toBe(en.sign_in.fallback_title);
  history.replaceState({}, '', '/sign-in?language=zh-cn');
  expect(authText('sign_in.fallback_title')).toBe(zh.sign_in.fallback_title);
  writeLocalePreferenceToStorage('en_US');
  expect(authText('sign_in.fallback_title')).toBe(en.sign_in.fallback_title);
  localStorage.clear();
  history.replaceState({}, '', '/sign-in?language=unsupported');
  expect(authText('sign_in.fallback_title')).toBe(en.sign_in.fallback_title);
  expect(
    authText('missing', { defaultValue: 'Hello {{name}}', name: '<Ada>' })
  ).toBe('Hello <Ada>');
  expect(authText('missing')).toBe('missing');
  expect(authText('missing', { defaultValue: '{{name}}', name: null })).toBe(
    ''
  );
  expect(
    authText('missing', { defaultValue: '{{name}}', name: undefined })
  ).toBe('');
  expect(authText('missing', { defaultValue: '{{unknown}}' })).toBe(
    '{{unknown}}'
  );
});
test('locale changes update mounted copy from the existing dictionaries', () => {
  function Copy() {
    const { t } = useAuthTranslation();
    return <p>{t('sign_in.fallback_title')}</p>;
  }
  render(<Copy />);
  expect(screen.getByText(en.sign_in.fallback_title)).toBeInTheDocument();
  act(() => writeLocalePreferenceToStorage('zh_Hans'));
  expect(screen.getByText(zh.sign_in.fallback_title)).toBeInTheDocument();
  localStorage.clear();
  act(() => {
    history.replaceState({}, '', '/sign-in?language=en');
    dispatchEvent(new PopStateEvent('popstate'));
  });
  expect(screen.getByText(en.sign_in.fallback_title)).toBeInTheDocument();
});
