import { beforeEach, expect, test } from 'vitest';
import { readCollapsedGroups, writeCollapsedGroups } from '../collapsed-groups';

beforeEach(() => localStorage.clear());

test('preserves legacy collapsed groups until the first versioned write', () => {
  localStorage.setItem('frontstage_collapsed_groups', JSON.stringify(['group']));
  const groups = readCollapsedGroups(localStorage);
  expect([...groups]).toEqual(['group']);
  groups.add('nested');
  writeCollapsedGroups(localStorage, groups);
  expect(JSON.parse(localStorage.getItem('frontstage_collapsed_groups:v1')!)).toEqual([
    'group', 'nested'
  ]);
  localStorage.setItem('frontstage_collapsed_groups', JSON.stringify(['stale']));
  expect([...readCollapsedGroups(localStorage)]).toEqual(['group', 'nested']);
});

test('retains an explicitly empty versioned selection instead of reviving legacy state', () => {
  localStorage.setItem('frontstage_collapsed_groups', JSON.stringify(['legacy']));
  writeCollapsedGroups(localStorage, new Set());
  expect([...readCollapsedGroups(localStorage)]).toEqual([]);
});

test('rejects malformed shapes and excludes invalid group identities', () => {
  localStorage.setItem('frontstage_collapsed_groups:v1', '{invalid');
  expect([...readCollapsedGroups(localStorage)]).toEqual([]);
  localStorage.setItem('frontstage_collapsed_groups:v1', JSON.stringify({ group: true }));
  expect([...readCollapsedGroups(localStorage)]).toEqual([]);
  localStorage.setItem('frontstage_collapsed_groups:v1', JSON.stringify(['group', 1, null, 'group']));
  expect([...readCollapsedGroups(localStorage)]).toEqual(['group']);
});
