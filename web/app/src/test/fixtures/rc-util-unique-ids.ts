import { afterEach, beforeEach, vi } from 'vitest';
import { useId } from 'react';

// rc-util deliberately returns one shared id under NODE_ENV=test. This collides
// across Radio.Group names and Select/Modal aria-labelledby targets. Opt-in
// integration fixtures use React's real ids while preserving explicit caller ids.
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

