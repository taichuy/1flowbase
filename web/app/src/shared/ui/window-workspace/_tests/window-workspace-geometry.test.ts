import { describe, expect, test } from 'vitest';

import { clampWindowWorkspaceRect } from '../window-workspace-geometry';

describe('Window Workspace geometry', () => {
  test('AC-010 keeps the title reachable without forcing the full window into the viewport', () => {
    expect(
      clampWindowWorkspaceRect(
        { left: 888, top: 1082, width: 384, height: 720 },
        360,
        320,
        { width: 1280, height: 900 }
      )
    ).toEqual({ left: 888, top: 852, width: 384, height: 720 });
  });
});
