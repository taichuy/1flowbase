import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import fs from 'node:fs';
import path from 'node:path';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const { getConsoleNavigation, patchUserPreferences } = vi.hoisted(() => ({
  getConsoleNavigation: vi.fn(),
  patchUserPreferences: vi.fn()
}));

vi.mock('@1flowbase/api-client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@1flowbase/api-client')>();
  return {
    ...actual,
    getConsoleNavigation
  };
});

vi.mock('../../shared/user-preferences/user-preferences', async () => {
  const actual = await vi.importActual<
    typeof import('../../shared/user-preferences/user-preferences')
  >('../../shared/user-preferences/user-preferences');

  return {
    ...actual,
    patchUserPreferences
  };
});

import { AppProviders } from '../../app/AppProviders';
import { resetAuthStore, useAuthStore } from '../../state/auth-store';
import {
  resetFrontstageDesignModeStore,
  useFrontstageDesignModeStore
} from '../../state/frontstage-design-mode-store';
import { AppShellFrame } from '../AppShellFrame';

function renderShell(pathname: string) {
  return render(
    <AppProviders>
      <AppShellFrame pathname={pathname}>
        <main>Content</main>
      </AppShellFrame>
    </AppProviders>
  );
}

describe('AppShellFrame', () => {
  beforeEach(() => {
    window.localStorage.clear();
    patchUserPreferences.mockReset();
    getConsoleNavigation.mockReset();
    getConsoleNavigation.mockResolvedValue({
      route_definitions: [
        {
          route_id: 'home',
          surface_key: 'home',
          path: '/',
          surface_kind: 'system'
        }
      ],
      navigation_items: [
        {
          item_id: 'home',
          route_id: 'home',
          parent_item_id: null,
          label_key: 'auto.workbench',
          navigation_slot: 'primary',
          order: 1
        }
      ],
      permission_bindings: []
    });
    patchUserPreferences.mockResolvedValue({
      id: 'user-1',
      account: 'root',
      name: 'Root',
      nickname: 'Root',
      email: 'root@example.com',
      phone: null,
      avatar_url: null,
      introduction: '',
      preferred_locale: null,
      effective_display_role: 'root',
      permissions: [],
      meta: {
        ui: {
          locale: {
            preferred_locale: 'en_US'
          }
        }
      }
    });
    resetAuthStore();
    resetFrontstageDesignModeStore();
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
        permissions: []
      }
    });
  });

  test('translates primary navigation labels at render time', async () => {
    renderShell('/');

    expect(await screen.findByText('workbench')).toBeInTheDocument();
    expect(screen.queryByText('auto.workbench')).not.toBeInTheDocument();
  });

  test('places the account menu after the secondary top actions', async () => {
    renderShell('/settings/data-models');

    await waitFor(() => {
      const accountLabel = screen.getByText('Root');
      const helpTrigger = screen.getByLabelText('help');

      expect(
        helpTrigger.compareDocumentPosition(accountLabel) &
          Node.DOCUMENT_POSITION_FOLLOWING
      ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    });
  });

  test('places the language switcher between help and account', async () => {
    renderShell('/settings/data-models');

    await waitFor(() => {
      const helpTrigger = screen.getByLabelText('help');
      const languageTrigger = screen.getByLabelText('Switch language');
      const accountLabel = screen.getByText('Root');

      expect(
        helpTrigger.compareDocumentPosition(languageTrigger) &
          Node.DOCUMENT_POSITION_FOLLOWING
      ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
      expect(
        languageTrigger.compareDocumentPosition(accountLabel) &
          Node.DOCUMENT_POSITION_FOLLOWING
      ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    });
  });

  test('does not mark the language switcher as a selected navigation item', async () => {
    const { container } = renderShell('/settings/data-models');

    expect(await screen.findByLabelText('Switch language')).toBeInTheDocument();
    expect(
      // Ant Design exposes submenu selected state only through its own classes.

      container.querySelector(
        '.app-shell-language-menu .ant-menu-submenu-selected'
      )
    ).not.toBeInTheDocument();
  });

  test('updates the current session locale from the language switcher', async () => {
    renderShell('/settings/data-models');

    fireEvent.mouseEnter(await screen.findByLabelText('Switch language'));
    fireEvent.click(await screen.findByText('English'));

    await waitFor(() => {
      expect(useAuthStore.getState().me?.preferred_locale).toBe('en_US');
    });
    expect(window.localStorage.getItem('1flowbase.ui.locale_preference')).toBe(
      'en_US'
    );
    expect(patchUserPreferences).toHaveBeenCalledWith(
      {
        ui: {
          locale: {
            preferred_locale: 'en_US'
          }
        }
      },
      'csrf-token'
    );
  });

  test('uses cached locale from localStorage when the profile has no locale preference', async () => {
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'en_US');
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
        preferred_locale: null,
        meta: {},
        effective_display_role: 'root',
        permissions: []
      }
    });

    renderShell('/settings/data-models');

    expect(await screen.findByLabelText('Switch language')).toBeInTheDocument();
  });

  test('places frontstage design mode icon before settings and toggles shared state', async () => {
    renderShell('/frontstage');

    await waitFor(() => {
      const settingsTrigger = screen.getByLabelText('settings');
      const designButton = screen.getByLabelText('Enter design mode');
      const helpTrigger = screen.getByLabelText('help');

      expect(
        designButton.compareDocumentPosition(settingsTrigger) &
          Node.DOCUMENT_POSITION_FOLLOWING
      ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
      expect(
        settingsTrigger.compareDocumentPosition(helpTrigger) &
          Node.DOCUMENT_POSITION_FOLLOWING
      ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
      expect(designButton).toHaveAttribute('aria-pressed', 'false');
    });

    fireEvent.click(screen.getByLabelText('Enter design mode'));

    expect(useFrontstageDesignModeStore.getState().isDesignMode).toBe(true);
    expect(screen.getByLabelText('Exit design mode')).toHaveAttribute(
      'aria-pressed',
      'true'
    );
  });

  test('renders frontstage design mode button globally on non-frontstage pages without navigating', async () => {
    const locationSpy = vi.fn();
    const originalLocation = window.location;

    // Mock window.location
    const mutableWindow = window as unknown as { location?: Location };
    delete mutableWindow.location;
    Object.defineProperty(window, 'location', {
      configurable: true,
      writable: true,
      value: {
        ...originalLocation,
        assign: vi.fn(),
        replace: vi.fn(),
        get href() {
          return 'http://localhost/';
        },
        set href(val: string) {
          locationSpy(val);
        },
        search: ''
      } as Location
    });

    renderShell('/');

    await waitFor(() => {
      const designButton = screen.getByLabelText('Enter design mode');
      expect(designButton).toBeInTheDocument();
    });

    fireEvent.click(screen.getByLabelText('Enter design mode'));

    expect(useFrontstageDesignModeStore.getState().isDesignMode).toBe(true);
    expect(locationSpy).not.toHaveBeenCalled();

    // restore
    Object.defineProperty(window, 'location', {
      configurable: true,
      writable: true,
      value: originalLocation
    });
  });

  test('keeps the top header to a single horizontally scrollable row', () => {
    const appShellCss = fs.readFileSync(
      path.resolve(import.meta.dirname, '../app-shell.css'),
      'utf8'
    );
    const headerRule = appShellCss.match(
      /\.app-shell-header\.ant-layout-header \{([\s\S]*?)\n\}/
    )?.[1];
    const actionRowRule = appShellCss.match(
      /\.app-shell-action-row\.ant-space \{([\s\S]*?)\n\}/
    )?.[1];
    const mobileActionsRule = appShellCss.match(
      /@media \(max-width: 767px\) \{[\s\S]*?\.app-shell-actions \{([\s\S]*?)\n {2}\}/
    )?.[1];
    const mobileHeaderMainRule = appShellCss.match(
      /@media \(max-width: 767px\) \{[\s\S]*?\.app-shell-header-main \{([\s\S]*?)\n {2}\}/
    )?.[1];
    const mobileNavigationRule = appShellCss.match(
      /@media \(max-width: 767px\) \{[\s\S]*?\.app-shell-navigation \{([\s\S]*?)\n {2}\}/
    )?.[1];
    const mobileMenuRule = appShellCss.match(
      /@media \(max-width: 767px\) \{[\s\S]*?\.app-shell-menu\.ant-menu-horizontal \{([\s\S]*?)\n {2}\}/
    )?.[1];

    expect(headerRule).toContain('flex-wrap: nowrap;');
    expect(headerRule).toContain('overflow-x: auto;');
    expect(headerRule).toContain('overflow-y: hidden;');
    expect(headerRule).toContain('white-space: nowrap;');
    expect(actionRowRule).toContain('flex-wrap: nowrap;');
    expect(appShellCss).not.toContain('flex-direction: column;');
    expect(appShellCss).not.toContain('height: auto;');
    expect(mobileActionsRule).toContain('align-self: center;');
    expect(mobileActionsRule).toContain('width: auto;');
    expect(mobileActionsRule).toContain('max-width: none;');
    expect(mobileHeaderMainRule).toContain('flex: none;');
    expect(mobileNavigationRule).toContain('flex: none;');
    expect(mobileNavigationRule).toContain('min-width: max-content;');
    expect(mobileMenuRule).toContain('flex: none;');
    expect(mobileMenuRule).toContain('min-width: max-content;');
  });
});
