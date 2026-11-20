import { expect, test, vi } from 'vitest';

const styles = vi.hoisted(() => ({
  raw: vi.fn(() => {
    throw new Error('unavailable raw stylesheet');
  }),
  inline: vi.fn(() => ({
    default: '.reactEasyCrop_Container{position:absolute}'
  }))
}));
vi.mock('react-easy-crop/react-easy-crop.css?raw', styles.raw);
vi.mock('react-easy-crop/react-easy-crop.css?inline', styles.inline);

test('the shared registry loads without crop CSS; only a crop block requests its isolated styles', async () => {
  const { createFrontstageNativeReactModuleRegistry } =
    await import('../registry');
  const registry = createFrontstageNativeReactModuleRegistry();
  await registry.load('react');
  expect(styles.raw).not.toHaveBeenCalled();
  expect(styles.inline).not.toHaveBeenCalled();

  await registry.load('antd-img-crop');
  const [asset] = await registry.resolveModuleAssets(['antd-img-crop']);
  expect(styles.inline).toHaveBeenCalledOnce();
  expect(asset?.role).toBe('shadow_style');
  expect(new TextDecoder().decode(asset?.bytes)).toContain(
    '.reactEasyCrop_Container'
  );
  expect(document.head.textContent).not.toContain('.reactEasyCrop_Container');
});
