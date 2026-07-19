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
    const listSpy = vi
      .spyOn(apiClient, 'listFrontstagePageTabs')
      .mockResolvedValue([]);
    const createSpy = vi
      .spyOn(apiClient, 'createFrontstagePageTab')
      .mockResolvedValue({} as never);
    const updateSpy = vi
      .spyOn(apiClient, 'updateFrontstagePageTab')
      .mockResolvedValue({} as never);
    const deleteSpy = vi
      .spyOn(apiClient, 'deleteFrontstagePageTab')
      .mockResolvedValue();

    try {
      await fetchFrontstagePageTabs('workspace-1', 'page-1');
      await createFrontstagePageTab(
        'workspace-1',
        'page-1',
        { title: '详情', route_segment: 'details', rank: '002000' },
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

      expect(listSpy).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        expect.any(String)
      );
      expect(createSpy).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        { title: '详情', route_segment: 'details', rank: '002000' },
        'csrf-123',
        expect.any(String)
      );
      expect(updateSpy).toHaveBeenNthCalledWith(
        1,
        'workspace-1',
        'page-1',
        'tab-2',
        { title: '明细' },
        'csrf-123',
        expect.any(String)
      );
      expect(updateSpy).toHaveBeenNthCalledWith(
        2,
        'workspace-1',
        'page-1',
        'tab-2',
        { rank: '001500' },
        'csrf-123',
        expect.any(String)
      );
      expect(deleteSpy).toHaveBeenCalledWith(
        'workspace-1',
        'page-1',
        'tab-2',
        'csrf-123',
        expect.any(String)
      );
    } finally {
      listSpy.mockRestore();
      createSpy.mockRestore();
      updateSpy.mockRestore();
      deleteSpy.mockRestore();
    }
  });
});
