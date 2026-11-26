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
      permissions: ['frontstage.page.design']
    }
  });
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
