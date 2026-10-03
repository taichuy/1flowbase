import { cleanup, render } from '@testing-library/react';
import { afterEach, expect, test } from 'vitest';
import VirtualList from '@rc-component/virtual-list';
import { compileNativeReactComponent } from '@1flowbase/page-runtime';
import { createFrontstageNativeReactModuleRegistry } from '../registry';

afterEach(cleanup);

test('virtual list compiles and loads the real virtualized component', async () => {
  const registry = createFrontstageNativeReactModuleRegistry();
  expect(
    compileNativeReactComponent(
      `import VirtualList from '@rc-component/virtual-list';
export default function Block() { return <VirtualList data={[]} height={400} itemHeight={47} itemKey="id">{item => item.id}</VirtualList>; }`,
      registry.definitions
    ).ok
  ).toBe(true);
  const loaded = await registry.load('@rc-component/virtual-list');
  expect(loaded.default).toBe(VirtualList);
  const data = Array.from({ length: 1000 }, (_, id) => ({ id }));
  const view = render(
    <VirtualList data={data} height={400} itemHeight={47} itemKey="id">
      {(item) => (
        <div data-testid="row" style={{ height: 47 }}>
          {item.id}
        </div>
      )}
    </VirtualList>
  );
  expect(view.getAllByTestId('row').length).toBeGreaterThan(0);
  expect(view.getAllByTestId('row').length).toBeLessThan(30);
  view.unmount();
  expect(view.container.childElementCount).toBe(0);
});
