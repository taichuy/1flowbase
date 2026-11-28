import { seedAccessFixture } from '../../test/fixtures/console-access';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const demand = vi.hoisted(() => ({
  topbarDesigner: vi.fn(),
  prepareStudio: vi.fn(async () => undefined)
}));

vi.mock('../TopbarNavigationDesigner', () => {
  demand.topbarDesigner();
  return {
    TopbarNavigationDesigner: () => null,
    TopbarNavigationItemLabel: () => null,
    TopbarNavigationMoveDialog: () => null
  };
});

vi.mock('../../features/frontstage/lib/jsx-studio/studio-demand', () => ({
  prepareFrontstageStudio: demand.prepareStudio,
  loadFrontstageStudio: vi.fn(async () => ({
    FrontstageJsxStudioDrawer: () => null
  })),
  loadFrontstageStudioRunPanel: vi.fn(async () => ({
    JsxStudioRunPanel: () => null
  }))
}));

import { resetAuthStore, useAuthStore } from '../../state/auth-store';
import {
  resetFrontstageDesignModeStore,
  useFrontstageDesignModeStore
} from '../../state/frontstage-design-mode-store';
import { preloadDesignModeDemand } from '../design-mode-demand';
import { FrontstageDesignModeAction } from '../FrontstageDesignModeAction';

function authenticateDesigner() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-token',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'root',
      name: 'Root',
      nickname: 'Root',
      email: 'root@example.com',
      phone: null,
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      policy_fingerprint: 'fixture-policy'
    }
  });
  seedAccessFixture(['frontstage.page.design']);
}

describe('design-mode-demand and FrontstageDesignModeAction', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    resetAuthStore();
    resetFrontstageDesignModeStore();
    authenticateDesigner();
  });

  test('hydrating persisted design mode on reload never eagerly preloads TopbarNavigationDesigner or JSX Studio', async () => {
    useFrontstageDesignModeStore.getState().setDesignMode(true);

    render(<FrontstageDesignModeAction />);
    const button = screen.getByRole('menuitemcheckbox');

    await act(async () => {
      fireEvent.pointerEnter(button);
      fireEvent.focus(button);
    });

    expect(button).toBeChecked();
    expect(demand.topbarDesigner).not.toHaveBeenCalled();
    expect(demand.prepareStudio).not.toHaveBeenCalled();
  });

  test('keeps persisted design mode while access is pending and clears it on backend denial', async () => {
    const state = useAuthStore.getState();
    state.setAuthenticated({ csrfToken: 'csrf-token', actor: { ...state.actor!, effective_display_role: 'member' }, me: null });
    useFrontstageDesignModeStore.getState().setDesignMode(true);
    let finish!: (response: Response) => void;
    const fetch = vi.spyOn(globalThis, 'fetch').mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    try {
      render(<FrontstageDesignModeAction />);
      expect(useFrontstageDesignModeStore.getState().isDesignMode).toBe(true);
      await act(async () => {
        await Promise.resolve();
        finish(new Response(JSON.stringify({ data: { policy_fingerprint: 'restricted', allowed: { 'frontstage.page.design': false } } }), { headers: { 'content-type': 'application/json' } }));
      });
      expect(useFrontstageDesignModeStore.getState().isDesignMode).toBe(false);
    } finally {
      fetch.mockRestore();
    }
  });

  test('hovering, focusing, or toggling design mode loads only TopbarNavigationDesigner and never prepares JSX Studio or Monaco', async () => {
    render(<FrontstageDesignModeAction />);
    const button = screen.getByRole('menuitemcheckbox');

    await act(async () => {
      fireEvent.pointerEnter(button);
      fireEvent.focus(button);
      fireEvent.click(button);
    });

    expect(useFrontstageDesignModeStore.getState().isDesignMode).toBe(true);
    expect(demand.topbarDesigner).toHaveBeenCalledTimes(1);
    expect(demand.prepareStudio).not.toHaveBeenCalled();

    await preloadDesignModeDemand();
    expect(demand.topbarDesigner).toHaveBeenCalledTimes(1);
    expect(demand.prepareStudio).not.toHaveBeenCalled();
  });
});
