import { afterEach, describe, expect, test, vi } from 'vitest';

import {
  authorizeConsoleExternalIdentity,
  authorizeConsolePasswordSetup,
  fetchConsoleAuthenticationMethods,
  setConsoleFirstPassword,
  unlinkConsoleExternalIdentity,
  patchConsoleMeMeta
} from '../../../console/identity/me';

describe('console me client', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  test('external identity authorization uses the current cookie session and CSRF', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          data: {
            authorization_url:
              'https://github.com/login/oauth/authorize?state=fixture'
          }
        }),
        { status: 200, headers: { 'content-type': 'application/json' } }
      )
    );
    await expect(
      authorizeConsoleExternalIdentity(
        { login_entry_id: 'entry-1' },
        'fixture-csrf',
        'https://flow.example'
      )
    ).resolves.toEqual({
      authorization_url:
        'https://github.com/login/oauth/authorize?state=fixture'
    });
    expect(fetchMock).toHaveBeenCalledWith(
      'https://flow.example/api/console/me/external-identities/authorize',
      expect.objectContaining({
        method: 'POST',
        credentials: 'include',
        body: JSON.stringify({ login_entry_id: 'entry-1' }),
        headers: expect.objectContaining({ 'x-csrf-token': 'fixture-csrf' })
      })
    );
  });

  test('patchConsoleMeMeta sends a merge patch to the me meta route', async () => {
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(
        JSON.stringify({
          data: {
            id: 'user-1',
            account: 'root',
            email: 'root@example.com',
            phone: null,
            nickname: 'Root',
            name: 'Root',
            avatar_url: null,
            introduction: '',
            preferred_locale: null,
            effective_display_role: 'root',
            permissions: [],
            meta: {
              ui: {
                data_tables: {
                  'applications.logs.runs': {
                    visibleColumnKeys: ['title', 'status']
                  }
                }
              }
            }
          },
          meta: null
        }),
        {
          status: 200,
          headers: { 'content-type': 'application/json' }
        }
      )
    );

    await expect(
      patchConsoleMeMeta(
        {
          ui: {
            data_tables: {
              'applications.logs.runs': {
                visibleColumnKeys: ['title', 'status']
              }
            }
          }
        },
        'csrf-123',
        'http://127.0.0.1:7800'
      )
    ).resolves.toMatchObject({
      meta: {
        ui: {
          data_tables: {
            'applications.logs.runs': {
              visibleColumnKeys: ['title', 'status']
            }
          }
        }
      }
    });

    expect(fetchMock).toHaveBeenCalledWith(
      'http://127.0.0.1:7800/api/console/me/meta',
      expect.objectContaining({
        method: 'PATCH',
        body: JSON.stringify({
          meta: {
            ui: {
              data_tables: {
                'applications.logs.runs': {
                  visibleColumnKeys: ['title', 'status']
                }
              }
            }
          }
        }),
        headers: expect.objectContaining({
          'content-type': 'application/json',
          'x-csrf-token': 'csrf-123'
        })
      })
    );
  });
  test('authentication methods preserves the Host lifecycle projection without inferring password or provider state', async () => {
    const methods = {
      password_configured: false,
      password_usable: false,
      password_setup_verified: true,
      password_verification_entries: [{ id: 'entry-1', title: 'GitHub' }],
      providers: [
        {
          auth_type: '1flowbase/github-login',
          display_name: 'GitHub',
          provider_available: false,
          login_entries: [],
          bindings: [
            {
              identity_id: 'identity-1',
              state: 'unlinked',
              subject_type: 'github_user_id',
              subject_value: '1234',
              issuer: 'https://github.com',
              account: 'fixture',
              nickname: 'Fixture',
              avatar_url: 'https://avatars.githubusercontent.com/u/1234',
              profile_url: 'https://github.com/fixture',
              bound_at: '2026-10-20T01:00:00Z',
              unlinked_at: '2026-10-20T02:00:00Z',
              provider_available: false,
              can_unlink: false,
              unlink_blocked_reason: 'provider_unavailable'
            }
          ]
        }
      ]
    };
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(
        new Response(JSON.stringify({ data: methods }), {
          status: 200,
          headers: { 'content-type': 'application/json' }
        })
      );
    await expect(
      fetchConsoleAuthenticationMethods('https://flow.example')
    ).resolves.toEqual(methods);
    expect(fetchMock).toHaveBeenCalledWith(
      'https://flow.example/api/console/me/authentication-methods',
      expect.objectContaining({ credentials: 'include' })
    );
  });

  test('unlink addresses the exact identity, encodes its path and requires CSRF', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(new Response(null, { status: 204 }));
    await expect(
      unlinkConsoleExternalIdentity(
        'identity/1',
        'fixture-csrf',
        'https://flow.example'
      )
    ).resolves.toBeUndefined();
    expect(fetchMock).toHaveBeenCalledWith(
      'https://flow.example/api/console/me/authentication-methods/external-identities/identity%2F1/unlink',
      expect.objectContaining({
        method: 'POST',
        credentials: 'include',
        headers: expect.objectContaining({ 'x-csrf-token': 'fixture-csrf' })
      })
    );
  });

  test('first-password verification uses its dedicated purpose endpoint and selected login entry', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(
        new Response(
          JSON.stringify({
            data: {
              authorization_url:
                'https://github.com/login/oauth/authorize?state=fixture'
            }
          }),
          { status: 200, headers: { 'content-type': 'application/json' } }
        )
      );
    await expect(
      authorizeConsolePasswordSetup(
        { login_entry_id: 'entry-1' },
        'fixture-csrf',
        'https://flow.example'
      )
    ).resolves.toEqual({
      authorization_url:
        'https://github.com/login/oauth/authorize?state=fixture'
    });
    expect(fetchMock).toHaveBeenCalledWith(
      'https://flow.example/api/console/me/authentication-methods/password/authorize',
      expect.objectContaining({
        method: 'POST',
        credentials: 'include',
        body: JSON.stringify({ login_entry_id: 'entry-1' }),
        headers: expect.objectContaining({ 'x-csrf-token': 'fixture-csrf' })
      })
    );
  });

  test('first-password write submits only the new password and consumes the no-content response', async () => {
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValue(new Response(null, { status: 204 }));
    await expect(
      setConsoleFirstPassword(
        { new_password: 'fixture-password-only' },
        'fixture-csrf',
        'https://flow.example'
      )
    ).resolves.toBeUndefined();
    expect(fetchMock).toHaveBeenCalledWith(
      'https://flow.example/api/console/me/authentication-methods/password',
      expect.objectContaining({
        method: 'POST',
        credentials: 'include',
        body: JSON.stringify({ new_password: 'fixture-password-only' }),
        headers: expect.objectContaining({ 'x-csrf-token': 'fixture-csrf' })
      })
    );
  });
});
