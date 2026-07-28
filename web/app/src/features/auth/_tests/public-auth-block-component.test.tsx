import { fireEvent, render, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import {
  compileNativeReactComponent,
  createNativeReactRuntimeFingerprint
} from '@1flowbase/page-runtime';

const { apiFetch } = vi.hoisted(() => ({ apiFetch: vi.fn() }));
vi.mock('@1flowbase/api-client', async () => {
  const actual = await vi.importActual<typeof import('@1flowbase/api-client')>(
    '@1flowbase/api-client'
  );
  return { ...actual, apiFetch };
});

import { PublicAuthBlock } from '../components/PublicAuthBlock';

describe('PublicAuthBlock Native Host composition', () => {
  beforeEach(() => apiFetch.mockReset());

  test('R6-AC-003 exposes a production fallback without editor diagnostics', async () => {
    const onAuthenticated = vi.fn();
    render(
      <PublicAuthBlock
        instance={instance('export default function Broken() {}')}
        onAuthenticated={onAuthenticated}
        nativeCompiler={vi.fn().mockResolvedValue({
          ok: false,
          diagnostics: [
            {
              phase: 'compile',
              code: 'syntax_invalid',
              path: 'source',
              message: 'Authenticator component is invalid'
            }
          ]
        })}
      />
    );

    const alert = await documentAlert();
    expect(alert).toHaveTextContent(
      'Failed to load sign-in options. Please refresh the page and try again.'
    );
    expect(alert).not.toHaveTextContent('Authenticator component is invalid');
    expectNoEditorDebugSurface();
    expect(onAuthenticated).not.toHaveBeenCalled();
  });

  test('R6-AC-003 retries production preparation without creating editor UI', async () => {
    const source = `
      export default function AuthFixture() {
        return <button type="button">Sign in</button>;
      }
    `;
    const nativeCompiler = vi
      .fn()
      .mockResolvedValueOnce({
        ok: false,
        diagnostics: [
          {
            phase: 'compile',
            code: 'syntax_invalid',
            path: 'source',
            message: 'temporary compile failure'
          }
        ]
      })
      .mockResolvedValueOnce(compiledResult(source));
    render(
      <PublicAuthBlock
        instance={instance(source)}
        onAuthenticated={vi.fn()}
        nativeCompiler={nativeCompiler}
      />
    );

    const alert = await documentAlert();
    fireEvent.click(within(alert).getByRole('button', { name: 'Retry' }));

    const shadow = await publicAuthShadow();
    expect(within(shadow).getByRole('button', { name: 'Sign in' })).toBeVisible();
    expect(nativeCompiler).toHaveBeenCalledTimes(2);
    expectNoEditorDebugSurface();
  });

  test('D4-AC-001/003/004 accepts only a canonical session response and keeps local state while the API is pending', async () => {
    const response = deferred<unknown>();
    apiFetch.mockReturnValue(response.promise);
    const onAuthenticated = vi.fn();
    const source = `
      import { useState } from 'react';
      import { Button } from 'antd';
      export default function AuthFixture({ ctx }) {
        const [count, setCount] = useState(0);
        return <div>
          <span data-testid="local-count">{count}</span>
          <Button onClick={() => setCount((value) => value + 1)}>Local</Button>
          <Button onClick={() => void ctx.api.post('/api/public/auth/sign-in')}>Sign in</Button>
        </div>;
      }
    `;
    render(
      <PublicAuthBlock
        instance={instance(source)}
        onAuthenticated={onAuthenticated}
        nativeCompiler={compiler(source)}
      />
    );
    const shadow = await publicAuthShadow();
    expectNoEditorDebugSurface();
    fireEvent.click(within(shadow).getByRole('button', { name: 'Sign in' }));
    fireEvent.click(within(shadow).getByRole('button', { name: 'Local' }));

    expect(within(shadow).getByTestId('local-count')).toHaveTextContent('1');
    expect(onAuthenticated).not.toHaveBeenCalled();
    response.resolve({
      csrf_token: 'csrf-1',
      effective_display_role: 'member',
      current_workspace_id: 'workspace-1'
    });
    await waitFor(() =>
      expect(onAuthenticated).toHaveBeenCalledWith({
        csrf_token: 'csrf-1',
        effective_display_role: 'member',
        current_workspace_id: 'workspace-1'
      })
    );
    expect(within(shadow).getByTestId('local-count')).toHaveTextContent('1');
  });

  test('D4-AC-002 keeps non-canonical and non-public responses from completing authentication', async () => {
    apiFetch.mockResolvedValue({ csrf_token: 'partial' });
    const onAuthenticated = vi.fn();
    const source = `
      import { Button } from 'antd';
      export default function AuthFixture({ ctx }) {
        return <Button onClick={() => void ctx.api.post('/api/public/auth/sign-in')}>Sign in</Button>;
      }
    `;
    render(
      <PublicAuthBlock
        instance={instance(source)}
        onAuthenticated={onAuthenticated}
        nativeCompiler={compiler(source)}
      />
    );
    const shadow = await publicAuthShadow();
    expectNoEditorDebugSurface();
    fireEvent.click(within(shadow).getByRole('button', { name: 'Sign in' }));
    await waitFor(() => expect(apiFetch).toHaveBeenCalledOnce());
    expect(onAuthenticated).not.toHaveBeenCalled();
  });
});

function instance(publicUiBlock: string) {
  return {
    id: 'auth-password-local',
    auth_type: 'password-local',
    title: 'Password',
    description: null,
    sort_order: 0,
    public_ui_block: publicUiBlock,
    public_variables: { self_registration_enabled: true }
  };
}

function compiler(source: string) {
  return vi.fn().mockResolvedValue(compiledResult(source));
}

function compiledResult(source: string) {
  const compiled = compileNativeReactComponent(
    source,
    [],
    createNativeReactRuntimeFingerprint('/auth-test-worker.js')
  );
  if (!compiled.ok) throw new Error(compiled.diagnostics[0]?.message);
  return {
    ok: true,
    artifact: compiled.artifact,
    diagnostics: []
  } as const;
}

async function publicAuthShadow(): Promise<HTMLElement> {
  await waitFor(() => {
    const shadow = document.querySelector(
      '[data-testid="native-react-public-auth-root"]'
    )?.shadowRoot;
    expect(shadow?.textContent).toContain('Sign in');
  });
  return document.querySelector(
    '[data-testid="native-react-public-auth-root"]'
  )!.shadowRoot as unknown as HTMLElement;
}

function expectNoEditorDebugSurface() {
  expect(
    document.querySelector('[data-testid="js-block-preview-console"]')
  ).toBeNull();
  expect(
    document.querySelector('[data-testid="js-block-console-pane"]')
  ).toBeNull();
  expect(
    document.querySelector('[data-testid="js-block-console-prompt"]')
  ).toBeNull();
  expect(document.querySelector('[role="separator"]')).toBeNull();
}

async function documentAlert(): Promise<HTMLElement> {
  let alert: HTMLElement | null = null;
  await waitFor(() => {
    alert = document.querySelector('[role="alert"]');
    expect(alert).not.toBeNull();
  });
  return alert!;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}
