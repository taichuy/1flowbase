import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import {
  compileNativeReactComponent,
  createNativeReactRuntimeFingerprint
} from '@1flowbase/page-runtime';

const { apiFetch, passwordSignIn } = vi.hoisted(() => ({
  apiFetch: vi.fn(),
  passwordSignIn: vi.fn()
}));
vi.mock('@1flowbase/api-client', async () => {
  const actual = await vi.importActual<typeof import('@1flowbase/api-client')>(
    '@1flowbase/api-client'
  );
  return { ...actual, apiFetch };
});
vi.mock('../api/session', async () => {
  const actual =
    await vi.importActual<typeof import('../api/session')>('../api/session');
  return { ...actual, signInWithPassword: passwordSignIn };
});

import { PublicAuthBlock } from '../components/PublicAuthBlock';
import { appI18n } from '../../../shared/i18n/app-i18n';

describe('PublicAuthBlock Native Host composition', () => {
  beforeEach(async () => {
    apiFetch.mockReset();
    passwordSignIn.mockReset();
    await appI18n.changeLanguage('en_US');
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test('AC-002 exposes the selector capability to the Block and handles its event', async () => {
    const requestSelector = vi.fn();
    const source = `
      export default function AuthFixture({ ctx }) {
        return <button
          type="button"
          onClick={() => ctx.events.emit('authenticator_selector_requested')}
        >
          Selector available: {String(ctx.inputs.authenticator_selection_available)}
        </button>;
      }
    `;
    render(
      <PublicAuthBlock
        instance={instance(source)}
        authenticatorSelector={{ request: requestSelector }}
        onAuthenticated={vi.fn()}
        nativeCompiler={compiler(source)}
      />
    );

    const shadow = await publicAuthShadow('Selector available: true');
    fireEvent.click(
      within(shadow).getByRole('button', { name: 'Selector available: true' })
    );
    expect(requestSelector).toHaveBeenCalledOnce();
  });

  test('AC-003 keeps the selector action inside the bundled fallback form', async () => {
    const requestSelector = vi.fn();
    const nativeCompiler = vi.fn().mockResolvedValue({
      ok: false,
      diagnostics: []
    });
    render(
      <PublicAuthBlock
        instance={instance('broken source')}
        authenticatorSelector={{ request: requestSelector }}
        onAuthenticated={vi.fn()}
        nativeCompiler={nativeCompiler}
      />
    );

    const fallback = await screen.findByTestId('builtin-password-sign-in');
    const backButton = within(fallback).getByRole('button', {
      name: 'Back to other sign-in options'
    });
    fireEvent.click(backButton);
    expect(requestSelector).toHaveBeenCalledOnce();
    expect(screen.getByRole('heading', { name: 'Welcome' })).toBeVisible();
  });

  test('AC-002 retries once before switching a builtin password authenticator to the bundled form', async () => {
    const onAuthenticated = vi.fn();
    const nativeCompiler = vi.fn().mockResolvedValue({
      ok: false,
      diagnostics: [
        {
          phase: 'compile',
          code: 'syntax_invalid',
          path: 'source',
          message: 'Authenticator component is invalid'
        }
      ]
    });
    render(
      <PublicAuthBlock
        instance={instance('export default function Broken() {}')}
        onAuthenticated={onAuthenticated}
        nativeCompiler={nativeCompiler}
      />
    );

    expect(
      await screen.findByRole('heading', { name: 'Welcome' })
    ).toBeVisible();
    expect(screen.getByLabelText('Account or email')).toBeVisible();
    expect(screen.getByLabelText('Password')).toBeVisible();
    expect(
      screen.queryByText('Authenticator component is invalid')
    ).not.toBeInTheDocument();
    expect(nativeCompiler).toHaveBeenCalledTimes(2);
    expectNoEditorDebugSurface();
    expect(onAuthenticated).not.toHaveBeenCalled();
  });

  test('AC-002 keeps the configured builtin password UI when its automatic retry succeeds', async () => {
    const source = `
      export default function AuthFixture() {
        return <button type="button">Configured sign in</button>;
      }
    `;
    const nativeCompiler = vi
      .fn()
      .mockResolvedValueOnce({ ok: false, diagnostics: [] })
      .mockResolvedValueOnce(compiledResult(source));
    render(
      <PublicAuthBlock
        instance={instance(source)}
        onAuthenticated={vi.fn()}
        nativeCompiler={nativeCompiler}
      />
    );

    const shadow = await publicAuthShadow('Configured sign in');
    expect(
      within(shadow).getByRole('button', { name: 'Configured sign in' })
    ).toBeVisible();
    expect(
      screen.queryByRole('heading', { name: 'Welcome' })
    ).not.toBeInTheDocument();
    expect(nativeCompiler).toHaveBeenCalledTimes(2);
  });

  test.each([
    ['non-builtin password', { is_builtin: false }],
    ['non-password', { is_builtin: true, auth_type: 'qr-code' }]
  ])(
    'AC-003 automatically retries a %s authenticator before using the configured UI',
    async (_case, overrides) => {
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
          instance={instance(source, overrides)}
          onAuthenticated={vi.fn()}
          nativeCompiler={nativeCompiler}
        />
      );

      const shadow = await publicAuthShadow();
      expect(
        within(shadow).getByRole('button', { name: 'Sign in' })
      ).toBeVisible();
      expect(nativeCompiler).toHaveBeenCalledTimes(2);
      expect(
        screen.queryByTestId('builtin-password-sign-in')
      ).not.toBeInTheDocument();
      expectNoEditorDebugSurface();
    }
  );

  test.each([
    ['non-builtin password', { is_builtin: false }],
    ['non-password', { is_builtin: true, auth_type: 'qr-code' }]
  ])(
    'AC-003 shows the escape form after a %s authenticator fails twice',
    async (_case, overrides) => {
      const nativeCompiler = vi.fn().mockResolvedValue({
        ok: false,
        diagnostics: []
      });
      render(
        <PublicAuthBlock
          instance={instance('broken source', overrides)}
          onAuthenticated={vi.fn()}
          nativeCompiler={nativeCompiler}
        />
      );

      expect(
        await screen.findByTestId('builtin-password-sign-in')
      ).toBeVisible();
      expect(nativeCompiler).toHaveBeenCalledTimes(2);
      expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    }
  );

  test('AC-003 switches to the bundled form after the preparation deadline and ignores a late compiler result', async () => {
    vi.useFakeTimers();
    const source = `
      export default function AuthFixture() {
        return <button type="button">Configured sign in</button>;
      }
    `;
    const preparation = deferred<ReturnType<typeof compiledResult>>();
    const nativeCompiler = vi.fn().mockReturnValue(preparation.promise);
    render(
      <PublicAuthBlock
        instance={instance(source)}
        onAuthenticated={vi.fn()}
        nativeCompiler={nativeCompiler}
      />
    );

    await act(async () => {
      await Promise.resolve();
      vi.advanceTimersByTime(10_000);
    });
    expect(
      screen.queryByRole('heading', { name: 'Welcome' })
    ).not.toBeInTheDocument();
    expect(nativeCompiler).toHaveBeenCalledTimes(2);

    await act(async () => {
      vi.advanceTimersByTime(10_000);
    });
    expect(screen.getByRole('heading', { name: 'Welcome' })).toBeVisible();

    preparation.resolve(compiledResult(source));
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.getByRole('heading', { name: 'Welcome' })).toBeVisible();
    expect(document.body).not.toHaveTextContent('Configured sign in');
  });

  test('AC-004 keeps the builtin fallback completely absent while the configured UI is healthy', async () => {
    const source = `
      export default function AuthFixture() {
        return <button type="button">Configured sign in</button>;
      }
    `;
    render(
      <PublicAuthBlock
        instance={instance(source)}
        onAuthenticated={vi.fn()}
        nativeCompiler={compiler(source)}
      />
    );

    const shadow = await publicAuthShadow('Configured sign in');
    expect(
      within(shadow).getByRole('button', { name: 'Configured sign in' })
    ).toBeVisible();
    expect(
      screen.queryByRole('button', { name: 'Use built-in sign-in' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('heading', { name: 'Welcome' })
    ).not.toBeInTheDocument();
  });

  test('AC-002 retries once after a builtin password Block throws during render', async () => {
    const source = `
      export default function BrokenAuthFixture() {
        throw new Error('render failed');
      }
    `;
    const nativeCompiler = compiler(source);
    render(
      <PublicAuthBlock
        instance={instance(source)}
        onAuthenticated={vi.fn()}
        nativeCompiler={nativeCompiler}
      />
    );

    expect(
      await screen.findByRole('heading', { name: 'Welcome' })
    ).toBeVisible();
    expect(nativeCompiler).toHaveBeenCalledTimes(2);
  });

  test('AC-006 submits the bundled form through the existing password sign-in flow', async () => {
    const session = {
      csrf_token: 'csrf-fallback',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-fallback'
    };
    passwordSignIn.mockResolvedValue(session);
    const onAuthenticated = vi.fn();
    const nativeCompiler = vi.fn().mockResolvedValue({
      ok: false,
      diagnostics: []
    });
    render(
      <PublicAuthBlock
        instance={instance('broken source')}
        onAuthenticated={onAuthenticated}
        nativeCompiler={nativeCompiler}
      />
    );

    fireEvent.change(await screen.findByLabelText('Account or email'), {
      target: { value: 'root' }
    });
    fireEvent.change(screen.getByLabelText('Password'), {
      target: { value: 'change-me' }
    });
    fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));

    await waitFor(() =>
      expect(passwordSignIn).toHaveBeenCalledWith({
        identifier: 'root',
        password: 'change-me'
      })
    );
    await waitFor(() => expect(onAuthenticated).toHaveBeenCalledWith(session));
    expect(nativeCompiler).toHaveBeenCalledTimes(2);
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

function instance(
  publicUiBlock: string,
  overrides: Partial<{
    is_builtin: boolean;
    auth_type: string;
  }> = {}
) {
  return {
    id: 'auth-password-local',
    auth_type: 'password-local',
    is_builtin: true,
    title: 'Password',
    description: null,
    sort_order: 0,
    public_ui_block: publicUiBlock,
    public_variables: { self_registration_enabled: true },
    ...overrides
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

async function publicAuthShadow(
  expectedText = 'Sign in'
): Promise<HTMLElement> {
  await waitFor(() => {
    const shadow = document.querySelector(
      '[data-testid="native-react-public-auth-root"]'
    )?.shadowRoot;
    expect(shadow?.textContent).toContain(expectedText);
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

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}
