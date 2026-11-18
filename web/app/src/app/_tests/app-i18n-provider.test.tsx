import { act, render, screen } from '@testing-library/react';
import { DatePicker } from 'antd';
import dayjs from 'dayjs';
import { useLayoutEffect } from 'react';
import { afterEach, expect, test, vi } from 'vitest';

import { AppI18nProvider } from '../AppI18nProvider';
import { writeLocalePreferenceToStorage } from '../../shared/user-preferences/locale-preference';

vi.mock('../../state/auth-store', () => ({
  useAuthStore: (selector: (state: { me: null }) => unknown) =>
    selector({ me: null })
}));

afterEach(() => {
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
