import { act, render, screen } from '@testing-library/react';
import { DatePicker } from 'antd';
import dayjs from 'dayjs';
import { useLayoutEffect, useState } from 'react';
import { afterEach, expect, test, vi } from 'vitest';

import { useTranslation } from 'react-i18next';
import * as i18nModule from '../../shared/i18n/app-i18n';
import { AppI18nProvider } from '../AppI18nProvider';
import { writeLocalePreferenceToStorage } from '../../shared/user-preferences/locale-preference';

vi.mock('../../state/auth-store', () => ({
  useAuthStore: (selector: (state: { me: null }) => unknown) =>
    selector({ me: null })
}));

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
  dayjs.locale('en');
});

test('date range calendar follows the app language, including months and weekdays', async () => {
  writeLocalePreferenceToStorage('zh_Hans');
  render(
    <AppI18nProvider>
      <DatePicker.RangePicker
        open
        defaultPickerValue={[dayjs('2026-11-04'), dayjs('2026-12-04')]}
      />
    </AppI18nProvider>
  );

  expect(await screen.findByText('11月')).toBeInTheDocument();
  expect(screen.getAllByText('一').length).toBeGreaterThan(0);
  expect(screen.queryByText('Nov')).not.toBeInTheDocument();

  await act(async () => writeLocalePreferenceToStorage('en_US'));
  expect(await screen.findByText('Nov')).toBeInTheDocument();
  expect(screen.getAllByText('Mo').length).toBeGreaterThan(0);
  expect(screen.queryByText('11月')).not.toBeInTheDocument();

  await act(async () => writeLocalePreferenceToStorage('zh_Hans'));
  expect(await screen.findByText('11月')).toBeInTheDocument();
});

test('locale preference written during child layout reaches the initial calendar', async () => {
  writeLocalePreferenceToStorage('en_US');
  function Calendar() {
    useLayoutEffect(() => {
      writeLocalePreferenceToStorage('zh_Hans');
    }, []);
    return (
      <DatePicker.RangePicker
        open
        defaultPickerValue={[dayjs('2026-11-04'), dayjs('2026-12-04')]}
      />
    );
  }
  render(
    <AppI18nProvider>
      <Calendar />
    </AppI18nProvider>
  );

  expect(await screen.findByText('11月')).toBeInTheDocument();
  expect(screen.getAllByText('一').length).toBeGreaterThan(0);
  expect(screen.queryByText('Nov')).not.toBeInTheDocument();
});

test('keeps an already displayed page and draft mounted while the next locale is loading', async () => {
  writeLocalePreferenceToStorage('en_US');
  await i18nModule.appI18n.changeLanguage('en_US');
  let mounts = 0;
  function DraftPage() {
    const [draft] = useState(() => {
      mounts += 1;
      return 'unsaved draft';
    });
    const { t } = useTranslation('frontstage');
    return (
      <div>
        <span>{draft}</span>
        <span>{t('auto.unnamed_page')}</span>
      </div>
    );
  }
  render(
    <AppI18nProvider>
      <DraftPage />
    </AppI18nProvider>
  );
  const draft = await screen.findByText('unsaved draft');
  const english = i18nModule.appI18n.t('auto.unnamed_page', {
    ns: 'frontstage',
    lng: 'en_US'
  });
  expect(screen.getByText(english)).toBeInTheDocument();
  let finish!: () => void;
  const blocked = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const prepare = i18nModule.prepareApplicationI18n;
  vi.spyOn(i18nModule, 'prepareApplicationI18n').mockImplementation(
    async (locale, path) => {
      await blocked;
      return prepare(locale, path);
    }
  );
  await act(async () => writeLocalePreferenceToStorage('zh_Hans'));
  expect(screen.getByText('unsaved draft')).toBe(draft);
  expect(screen.getByText(english)).toBeInTheDocument();
  expect(mounts).toBe(1);
  await act(async () => finish());
  const chinese = i18nModule.appI18n.t('auto.unnamed_page', {
    ns: 'frontstage',
    lng: 'zh_Hans'
  });
  expect(await screen.findByText(chinese)).toBeInTheDocument();
  expect(screen.getByText('unsaved draft')).toBe(draft);
  expect(mounts).toBe(1);
});
