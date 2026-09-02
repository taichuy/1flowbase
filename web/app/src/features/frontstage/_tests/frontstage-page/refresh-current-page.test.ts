import { expect, test, vi } from 'vitest';

import { refreshCurrentFrontstagePage } from '../../pages/frontstage-page/refresh-current-page';

test('#1975 AC-002 refreshes the active page dependency closure', async () => {
  const refreshPageContent = vi.fn(() => Promise.resolve());
  const refreshBlockRoots = vi.fn(() => Promise.resolve());
  const refreshBlockRuntimeAssembly = vi.fn(() => Promise.resolve());

  await refreshCurrentFrontstagePage({
    refreshPageContent,
    refreshBlockRoots,
    refreshBlockRuntimeAssembly
  });

  expect(refreshPageContent).toHaveBeenCalledTimes(1);
  expect(refreshBlockRoots).toHaveBeenCalledTimes(1);
  expect(refreshBlockRuntimeAssembly).toHaveBeenCalledTimes(1);
});

test('#1975 AC-002 does not require a runtime assembly outside a block route', async () => {
  const refreshPageContent = vi.fn(() => Promise.resolve());
  const refreshBlockRoots = vi.fn(() => Promise.resolve());

  await refreshCurrentFrontstagePage({
    refreshPageContent,
    refreshBlockRoots
  });

  expect(refreshPageContent).toHaveBeenCalledTimes(1);
  expect(refreshBlockRoots).toHaveBeenCalledTimes(1);
});

test('#1975 AC-003 keeps the refresh pending until every query settles', async () => {
  let finishBlockRoots: () => void = () => undefined;
  const refreshBlockRoots = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        finishBlockRoots = resolve;
      })
  );
  const refresh = refreshCurrentFrontstagePage({
    refreshPageContent: () => Promise.reject(new Error('content unavailable')),
    refreshBlockRoots
  });
  let settled = false;
  void refresh.then(
    () => {
      settled = true;
    },
    () => {
      settled = true;
    }
  );

  await Promise.resolve();
  expect(settled).toBe(false);

  finishBlockRoots();
  await expect(refresh).rejects.toThrow('content unavailable');
  expect(settled).toBe(true);
});
