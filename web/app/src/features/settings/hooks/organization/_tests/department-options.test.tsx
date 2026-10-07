import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';
const fetchDepartments = vi.hoisted(() => vi.fn());
vi.mock('../../../api/departments', () => ({
  fetchSettingsDepartments: fetchDepartments,
  settingsDepartmentsQueryKey: ['settings', 'departments']
}));
import { AppProviders } from '../../../../../app/AppProviders';
import { useDepartmentOptions } from '../useDepartmentOptions';
const item = (id: string, is_match = true) => ({
  id,
  name: id,
  parent_id: null,
  role_codes: [],
  member_count: 0,
  is_match,
  has_children: false
});
describe('remote department options', () => {
  test('resolves selections separately, excludes search context and fetches another option page on demand', async () => {
    fetchDepartments.mockImplementation(async ({ ids, prefix, cursor }) => {
      if (ids)
        return {
          items: [item('selected')],
          has_more: false,
          next_cursor: null
        };
      if (prefix)
        return {
          items: cursor
            ? [item('match2')]
            : [item('context', false), item('match1')],
          has_more: !cursor,
          next_cursor: cursor ? null : 'next'
        };
      return { items: [item('root')], has_more: false, next_cursor: null };
    });
    const { result } = renderHook(() => useDepartmentOptions(['selected']), {
      wrapper: AppProviders
    });
    await waitFor(() =>
      expect(result.current.departments.map((row) => row.id)).toEqual([
        'selected',
        'root'
      ])
    );
    act(() => result.current.setPrefix('match'));
    await waitFor(() =>
      expect(result.current.departments.map((row) => row.id)).toEqual([
        'selected',
        'match1'
      ])
    );
    expect(fetchDepartments).not.toHaveBeenCalledWith({
      prefix: 'match',
      limit: 50,
      cursor: 'next'
    });
    act(() => result.current.loadMore());
    await waitFor(() =>
      expect(result.current.departments.map((row) => row.id)).toEqual([
        'selected',
        'match1',
        'match2'
      ])
    );
    expect(fetchDepartments).toHaveBeenCalledWith({
      ids: 'selected',
      limit: 50
    });
  });
});
