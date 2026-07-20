import { describe, expect, test } from 'vitest';

import {
  activateWindowWorkspaceEntry,
  closeWindowWorkspaceEntry,
  createWindowWorkspaceState,
  openWindowWorkspaceEntry,
  setWindowWorkspaceDirty,
  toggleWindowWorkspaceMaximized
} from '../window-workspace-state';

const rect = { left: 100, top: 80, width: 800, height: 600 };

describe('Window Workspace state', () => {
  test('AC-010 enforces singleton ids, activation, collision offset and maximize restore', () => {
    let state = openWindowWorkspaceEntry(createWindowWorkspaceState(), {
      id: 'studio',
      owner: 'frontstage',
      parent_id: null,
      rect,
      dirty: false
    });
    state = openWindowWorkspaceEntry(state, {
      id: 'inspector',
      owner: 'frontstage',
      parent_id: 'studio',
      rect,
      dirty: false
    });
    expect(state.windows[1].rect).toMatchObject({ left: 124, top: 104 });
    const singleton = openWindowWorkspaceEntry(state, {
      id: 'studio',
      owner: 'frontstage',
      parent_id: null,
      rect,
      dirty: false
    });
    expect(singleton.windows).toHaveLength(2);
    expect(
      singleton.windows.find((window) => window.id === 'studio')?.z_index
    ).toBeGreaterThan(
      singleton.windows.find((window) => window.id === 'inspector')?.z_index ??
        0
    );
    const maximized = toggleWindowWorkspaceMaximized(singleton, 'studio', {
      left: 8,
      top: 8,
      width: 1200,
      height: 800
    });
    expect(maximized.windows[0].maximized).toBe(true);
    expect(
      toggleWindowWorkspaceMaximized(maximized, 'studio', rect).windows[0].rect
    ).toEqual(rect);
  });

  test('AC-010 cascades child close and exposes dirty descendants to the caller', () => {
    let state = openWindowWorkspaceEntry(createWindowWorkspaceState(), {
      id: 'studio',
      owner: 'frontstage',
      parent_id: null,
      rect,
      dirty: false
    });
    state = openWindowWorkspaceEntry(state, {
      id: 'console',
      owner: 'frontstage',
      parent_id: 'studio',
      rect,
      dirty: false
    });
    state = setWindowWorkspaceDirty(state, 'console', true);
    state = activateWindowWorkspaceEntry(state, 'console');
    const closed = closeWindowWorkspaceEntry(state, 'studio');
    expect(closed.state.windows).toEqual([]);
    expect(closed.closed).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 'console', dirty: true })
      ])
    );
  });
});
