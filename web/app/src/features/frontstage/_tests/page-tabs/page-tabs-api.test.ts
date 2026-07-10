import * as apiClient from '@1flowbase/api-client';
import { describe, expect, test, vi } from 'vitest';

import {
  createFrontstagePageTab,
  deleteFrontstagePageTab,
  fetchFrontstagePageTabs,
  moveFrontstagePageTab,
  renameFrontstagePageTab
} from '../../api/page-tabs';

describe('frontstage page tab feature api', () => {
  test('AC-005/006 uses page and tab scoped endpoints with backend field names', async () => {
    const apiFetchSpy = vi.spyOn(apiClient, 'apiFetch');
    apiFetchSpy
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce({ id: 'tab-2' })
      .mockResolvedValueOnce({ id: 'tab-2' })
      .mockResolvedValueOnce({ id: 'tab-2' })
      .mockResolvedValueOnce(undefined);

    try {
      await fetchFrontstagePageTabs('workspace-1', 'page-1');
      await createFrontstagePageTab(
        'workspace-1',
        'page-1',
        { title: '详情', rank: '002000' },
        'csrf-123'
      );
      await renameFrontstagePageTab(
        'workspace-1',
        'page-1',
        'tab-2',
        { title: '明细' },
        'csrf-123'
      );
      await moveFrontstagePageTab(
        'workspace-1',
        'page-1',
        'tab-2',
        { rank: '001500' },
        'csrf-123'
      );
      await deleteFrontstagePageTab(
        'workspace-1',
        'page-1',
        'tab-2',
        'csrf-123'
      );

      expect(apiFetchSpy).toHaveBeenNthCalledWith(2, {
        path: '/api/console/frontstage/workspace-1/pages/page-1/tabs',
        method: 'POST',
        body: { title: '详情', rank: '002000' },
        csrfToken: 'csrf-123',
        baseUrl: expect.any(String)
      });
      expect(apiFetchSpy).toHaveBeenNthCalledWith(3, {
        path: '/api/console/frontstage/workspace-1/pages/page-1/tabs/tab-2',
        method: 'PATCH',
        body: { title: '明细' },
        csrfToken: 'csrf-123',
        baseUrl: expect.any(String)
      });
      expect(apiFetchSpy).toHaveBeenNthCalledWith(4, {
        path: '/api/console/frontstage/workspace-1/pages/page-1/tabs/tab-2',
        method: 'PATCH',
        body: { rank: '001500' },
        csrfToken: 'csrf-123',
        baseUrl: expect.any(String)
      });
    } finally {
      apiFetchSpy.mockRestore();
    }
  });
});
