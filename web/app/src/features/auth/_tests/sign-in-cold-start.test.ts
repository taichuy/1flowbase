import { readFileSync } from 'node:fs';
import path from 'node:path';
import { afterEach, expect, test, vi } from 'vitest';
import { createFrontstageNativeReactModuleRegistry } from '../../frontstage/lib/native-modules/registry';
import { createDefaultLoginRegistry } from '../lib/default-login-registry';
import { matchingDefaultLoginArtifact } from '../lib/default-login-artifact';
import {
  consumeLoginEntryDiscovery,
  discardLoginEntryDiscovery,
  startLoginEntryDiscovery
} from '../api/login-entry-discovery';
import { fetchLoginEntries } from '../api/session';
vi.mock('../api/session', () => ({ fetchLoginEntries: vi.fn() }));
afterEach(() => {
  discardLoginEntryDiscovery();
  vi.clearAllMocks();
});

test('only exact shipped source with compatible runtime bindings reuses the compiled program', () => {
  const source = readFileSync(
    path.resolve(
      '../../api/crates/control-plane/src/auth/public_ui_blocks/password_local.tsx'
    ),
    'utf8'
  );
  const definitions = createFrontstageNativeReactModuleRegistry().definitions;
  expect(matchingDefaultLoginArtifact(source, definitions)).not.toBeNull();
  expect(matchingDefaultLoginArtifact(source + '\n', definitions)).toBeNull();
  expect(
    matchingDefaultLoginArtifact(
      source,
      definitions.filter((d) => d.module_source !== 'antd')
    )
  ).toBeNull();
});

test('the small registry supplies the actual host components and does not advertise unrelated exports', async () => {
  const slim = createDefaultLoginRegistry();
  const full = createFrontstageNativeReactModuleRegistry();
  for (const entry of slim.definitions) {
    const [smallModule, wholeModule] = await Promise.all([
      slim.load(entry.module_source),
      full.load(entry.module_source)
    ]);
    for (const name of entry.exports) {
      // The full icon root wraps each leaf in React.lazy. The small registry
      // supplies that exact leaf immediately, not a substitute SVG.
      const original =
        entry.module_source === '@ant-design/icons'
          ? (await full.load(`@ant-design/icons/${name}`)).default
          : wholeModule[name];
      expect(smallModule[name]).toBe(original);
    }
  }
  expect((await slim.load('antd')).Table).toBeUndefined();
});

test('an early resolved response is handed to one visit, never reused on revisit', async () => {
  const payload = {
    default_login_entry_id: 'password-local',
    login_entries: []
  };
  vi.mocked(fetchLoginEntries).mockResolvedValue(payload);
  startLoginEntryDiscovery();
  startLoginEntryDiscovery();
  await Promise.resolve();
  await expect(consumeLoginEntryDiscovery()).resolves.toBe(payload);
  expect(fetchLoginEntries).toHaveBeenCalledTimes(1);
  await consumeLoginEntryDiscovery();
  expect(fetchLoginEntries).toHaveBeenCalledTimes(2);
});

test('failed discovery reaches the page error handler and the next visit can retry', async () => {
  vi.mocked(fetchLoginEntries)
    .mockRejectedValueOnce(new Error('offline'))
    .mockResolvedValueOnce({
      default_login_entry_id: 'password-local',
      login_entries: []
    });
  startLoginEntryDiscovery();
  await expect(consumeLoginEntryDiscovery()).rejects.toThrow('offline');
  await expect(consumeLoginEntryDiscovery()).resolves.toEqual({
    default_login_entry_id: 'password-local',
    login_entries: []
  });
});
