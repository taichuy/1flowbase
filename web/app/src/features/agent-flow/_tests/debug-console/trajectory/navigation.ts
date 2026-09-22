import { afterEach, beforeEach, vi } from 'vitest';
import { useId } from 'react';

// rc-util deliberately returns the same id for every component under NODE_ENV=test.
// A page Select then steals Modal's aria-labelledby target. Keep production-like
// React ids in these trajectory integration fixtures; retain accessible-name queries.
const { rcUseIdModule, rcUseIdCjs } = await vi.hoisted(async () => {
  const { createRequire } = await import('node:module');
  const require = createRequire(import.meta.url);
  return {
    rcUseIdCjs: require(
      require.resolve('@rc-component/util/lib/hooks/useId.js', {
        paths: [require.resolve('antd')]
      })
    ) as { default: (id?: string) => string },
    rcUseIdModule: require.resolve('@rc-component/util/es/hooks/useId.js', {
      paths: [require.resolve('antd')]
    })
  };
});
vi.mock(rcUseIdModule, async (importOriginal) => {
  const original = await importOriginal<Record<string, unknown>>();
  const { useId } = await import('react');
  return {
    ...original,
    default: function useFixtureId(id?: string) {
      const reactId = useId();
      return id || reactId;
    }
  };
});

// Externalized antd consumers use the CJS entry; Vite-transformed consumers use
// the ESM mock above. Restore the CJS hook after each test, without global setup.
let restoreCjsId: (() => void) | undefined;
beforeEach(() => {
  const mock = vi
    .spyOn(rcUseIdCjs, 'default')
    .mockImplementation(function useFixtureId(id?: string) {
      const reactId = useId();
      return id || reactId;
    });
  restoreCjsId = () => mock.mockRestore();
});
afterEach(() => restoreCjsId?.());

import { fireEvent, within } from '@testing-library/react';

export async function openPayloadSection(
  nodeDetail: HTMLElement,
  section: '输入' | '数据处理' | '输出'
) {
  fireEvent.click(
    await within(nodeDetail).findByText(section, { exact: true })
  );
}
