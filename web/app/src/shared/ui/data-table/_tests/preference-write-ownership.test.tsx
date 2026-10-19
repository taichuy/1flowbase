import { act, renderHook } from '@testing-library/react';
import type { ConsoleMe } from '@1flowbase/api-client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import {
  usePersistedDataTableConfiguration,
  type DataTableColumn
} from '../data-table-state';
import { useUserPreferenceDataTableConfiguration } from '../user-preference-data-table';

const { patchUserPreferences } = vi.hoisted(() => ({
  patchUserPreferences: vi.fn()
}));
vi.mock('../../../user-preferences/user-preferences', () => ({
  patchUserPreferences
}));

const columns: DataTableColumn<{ name: string; owner: string }>[] = [
  { key: 'name', title: 'Name', width: 180 },
  { key: 'owner', title: 'Owner', width: 140 }
];
const table = (visibleColumnKeys: string[]) => ({
  visibleColumnKeys,
  columnWidths: { name: 180, owner: 140 }
});
const profile = (visibleColumnKeys: string[]): ConsoleMe => ({
  id: 'user-1',
  account: 'member',
  email: 'member@example.com',
  phone: null,
  nickname: 'Member',
  name: 'Member',
  avatar_url: null,
  introduction: '',
  effective_display_role: 'member',
  permissions: [],
  meta: { ui: { data_tables: { first: table(visibleColumnKeys) } } }
});

describe('data table preference write ownership', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    resetAuthStore();
    window.localStorage.clear();
    patchUserPreferences.mockReset();
    useAuthStore.setState({ csrfToken: 'csrf', me: profile(['name']) });
  });
  afterEach(() => {
    vi.useRealTimers();
    resetAuthStore();
  });

  it('reads another local table without overwriting its saved configuration', () => {
    window.localStorage.setItem('first', JSON.stringify(table(['name'])));
    window.localStorage.setItem('second', JSON.stringify(table(['owner'])));
    const write = vi.spyOn(Storage.prototype, 'setItem');
    const { result, rerender } = renderHook(
      ({ storageKey }) =>
        usePersistedDataTableConfiguration({ storageKey, columns }),
      { initialProps: { storageKey: 'first' } }
    );
    expect(result.current.visibleColumnKeys).toEqual(['name']);
    rerender({ storageKey: 'second' });
    expect(result.current.visibleColumnKeys).toEqual(['owner']);
    expect(JSON.parse(window.localStorage.getItem('second')!)).toEqual(
      table(['owner'])
    );
    expect(write).not.toHaveBeenCalled();
    act(() => {
      result.current.setVisibleColumnKeys(['name', 'owner']);
      result.current.setColumnWidths((widths) => ({ ...widths, name: 260 }));
    });
    expect(JSON.parse(window.localStorage.getItem('second')!)).toEqual({
      visibleColumnKeys: ['name', 'owner'],
      columnWidths: { name: 260, owner: 140 }
    });
    expect(JSON.parse(window.localStorage.getItem('first')!)).toEqual(
      table(['name'])
    );
    write.mockRestore();
  });

  it('hydrates server meta without PATCH and saves a user edit once', async () => {
    patchUserPreferences.mockResolvedValue(profile(['name', 'owner']));
    const { result } = renderHook(() =>
      useUserPreferenceDataTableConfiguration({
        preferenceKey: 'first',
        columns
      })
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(patchUserPreferences).not.toHaveBeenCalled();
    act(() => useAuthStore.getState().setMe(profile(['owner'])));
    expect(result.current.visibleColumnKeys).toEqual(['owner']);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(patchUserPreferences).not.toHaveBeenCalled();
    act(() => result.current.setVisibleColumnKeys((keys) => ['name', ...keys]));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(patchUserPreferences).toHaveBeenCalledWith(
      { ui: { data_tables: { first: table(['name', 'owner']) } } },
      'csrf'
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(patchUserPreferences).toHaveBeenCalledTimes(1);
    expect(result.current.visibleColumnKeys).toEqual(['name', 'owner']);
  });

  it('cancels a pending edit when the table owner changes', async () => {
    const { result, rerender } = renderHook(
      ({ preferenceKey }) =>
        useUserPreferenceDataTableConfiguration({ preferenceKey, columns }),
      { initialProps: { preferenceKey: 'first' } }
    );
    act(() => result.current.setVisibleColumnKeys(['owner']));
    rerender({ preferenceKey: 'second' });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(patchUserPreferences).not.toHaveBeenCalled();
    expect(result.current.visibleColumnKeys).toEqual(['name', 'owner']);
  });
});
