import { afterEach, describe, expect, test, vi } from 'vitest';

import { patchConsoleMeMeta } from '../../../console/identity/me';

describe('console me client', () => {
  afterEach(() => {
    vi.restoreAllMocks();
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
});
