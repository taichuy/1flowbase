import { renderHook } from '@testing-library/react';
import { expect, test, vi } from 'vitest';
import { useFrontstageRuntimeAssembly } from '../../../hooks/use-frontstage-runtime-assembly';

const native = vi.hoisted(() => ({
  prepare: vi.fn(() => ({ preparations: {}, isValidating: false }))
}));
vi.mock('../../../hooks/use-frontstage-page-canvas-native-preparations', () => ({
  useFrontstagePageCanvasNativePreparations: native.prepare
}));

// The native hook's real suspend/revalidate/error tests cover the lifecycle;
// this boundary regression ensures assembly pages participate in that owner.
test('propagates assembly visibility to the native lifecycle owner', () => {
  const { rerender } = renderHook(
    ({ active }) =>
      useFrontstageRuntimeAssembly({
        active,
        workspaceId: 'workspace-1',
        pageId: 'page-1',
        assembly: undefined
      }),
    { initialProps: { active: true } }
  );
  expect(native.prepare).toHaveBeenLastCalledWith(
    expect.objectContaining({ active: true })
  );
  rerender({ active: false });
  expect(native.prepare).toHaveBeenLastCalledWith(
    expect.objectContaining({ active: false })
  );
  rerender({ active: true });
  expect(native.prepare).toHaveBeenLastCalledWith(
    expect.objectContaining({ active: true })
  );
});
