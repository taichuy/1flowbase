import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const { suspendedPreview } = vi.hoisted(() => ({
  suspendedPreview: new Promise<never>(() => undefined)
}));

vi.mock(
  '../../components/embedded-assistant/EmbeddedAgentAssistantPreview',
  () => ({
    EmbeddedAgentAssistantPreview: () => {
      throw suspendedPreview;
    }
  })
);

import { AppProviders } from '../../../../app/AppProviders';
import { i18nText } from '../../../../shared/i18n/text';
import { EmbeddedAgentAssistant } from '../../components/embedded-assistant/EmbeddedAgentAssistant';

describe('EmbeddedAgentAssistant loading window', () => {
  beforeEach(() => {
    vi.spyOn(window, 'innerHeight', 'get').mockReturnValue(997);
    vi.spyOn(window, 'innerWidth', 'get').mockReturnValue(887);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  test('AC-001/002 portals the cold-load frame outside a filtered header with a controlled rect', async () => {
    const { container } = render(
      <AppProviders>
        <header style={{ backdropFilter: 'blur(20px)', height: 56 }}>
          <EmbeddedAgentAssistant />
        </header>
      </AppProviders>
    );

    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );

    const loadingWindow = await screen.findByTestId(
      'embedded-agent-assistant-window-shell'
    );

    expect(loadingWindow.parentElement).toBe(document.body);
    expect(
      container.querySelector('[data-testid="embedded-agent-assistant-window-shell"]')
    ).toBeNull();
    await waitFor(() => {
      expect(Number.parseFloat(loadingWindow.style.left)).toBeGreaterThanOrEqual(
        0
      );
      expect(Number.parseFloat(loadingWindow.style.top)).toBeGreaterThanOrEqual(
        0
      );
      expect(Number.parseFloat(loadingWindow.style.width)).toBeGreaterThan(0);
      expect(Number.parseFloat(loadingWindow.style.height)).toBeGreaterThan(0);
    });
  });
});
