import { describe, expect, test } from 'vitest';

import {
  clampWindowWorkspaceRect,
  fitWindowWorkspaceRect
} from '../window-workspace-geometry';

describe('Window Workspace geometry', () => {
  test('AC-010 keeps the title reachable without forcing the full window into the viewport', () => {
    expect(
      clampWindowWorkspaceRect(
        { left: 888, top: 1082, width: 384, height: 720 },
        360,
        320,
        { left: 0, top: 0, width: 1280, height: 900 }
      )
    ).toEqual({ left: 888, top: 852, width: 384, height: 720 });
  });

  test('fits an opening desktop window below the app header with viewport margins', () => {
    expect(
      fitWindowWorkspaceRect(
        { left: 120, top: 64, width: 1080, height: 760 },
        320,
        320,
        { left: 0, top: 56, width: 1400, height: 744 }
      )
    ).toEqual({ left: 120, top: 64, width: 1080, height: 728 });
  });

  test('fits an opening mobile window inside the narrow viewport below the app header', () => {
    expect(
      fitWindowWorkspaceRect(
        { left: 120, top: 64, width: 1080, height: 760 },
        320,
        320,
        { left: 0, top: 56, width: 390, height: 744 }
      )
    ).toEqual({ left: 8, top: 64, width: 374, height: 728 });
  });
});
