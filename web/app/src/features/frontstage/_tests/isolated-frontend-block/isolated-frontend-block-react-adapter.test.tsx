import { act, cleanup, render, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import type { PreparedFrontstageIsolatedContribution } from '../../lib/isolated-frontend-block-contribution';
import { FrontstageIsolatedFrontendBlockHost } from '../../lib/isolated-frontend-block-react-adapter';

const PROGRAM_SOURCE = `globalThis.__oneflowbaseIsolatedBlock = {
  mount(root, props) {
    root.textContent = String(props.label);
    return { update(next) { root.textContent = String(next.label); } };
  }
};`;

// The runtime's existing realm tests use this transport shape. Keep the real
// adapter, realm lifecycle, iframe and message handlers in this regression.
class FakeMessagePort extends EventTarget {
  peer: FakeMessagePort | null = null;
  readonly sent: unknown[] = [];
  closed = false;

  postMessage(message: unknown): void {
    this.sent.push(message);
    this.peer?.dispatchEvent(new MessageEvent('message', { data: message }));
  }

  start(): void {}

  close(): void {
    this.closed = true;
  }
}

class FakeMessageChannel {
  static readonly instances: FakeMessageChannel[] = [];
  readonly port1 = new FakeMessagePort();
  readonly port2 = new FakeMessagePort();

  constructor() {
    this.port1.peer = this.port2;
    this.port2.peer = this.port1;
    FakeMessageChannel.instances.push(this);
  }
}

function preparation(
  props: Record<string, unknown>,
  contributionId = 'frontend-block.installation-1.isolated-chart',
  source = PROGRAM_SOURCE
): PreparedFrontstageIsolatedContribution {
  return {
    state: 'prepared',
    blockInstanceId: 'block-1',
    contributionId,
    blockId: 'installation-1:isolated-chart',
    blockVersion: '1.0.0',
    graphFingerprint: 'graph-fingerprint',
    runtimeKind: 'isolated',
    executionKind: 'ui_mount',
    isolationRequirement: 'independent_realm',
    lifecycleKind: 'workspace_assignment',
    grantedPermissions: ['frontend-block.ui-mount.isolated-realm'],
    assetIntegrity: 'verified_sha256',
    program: { source, props }
  };
}

function setup(onRuntimeError = vi.fn()) {
  const root = document.createElement('div');
  document.body.append(root);
  const initial = preparation({ label: 'initial' });
  const view = render(
    <FrontstageIsolatedFrontendBlockHost
      root={root}
      preparation={initial}
      onRuntimeError={onRuntimeError}
    />
  );
  return { ...view, root, initial, onRuntimeError };
}

async function pendingRealm(root: Element) {
  await waitFor(() => expect(root.querySelector('iframe')).not.toBeNull());
  const iframe = root.querySelector('iframe');
  const channel = FakeMessageChannel.instances.at(-1);
  if (!iframe || !channel) throw new Error('realm transport was not created');
  return { iframe, channel };
}

async function mountRealm(root: Element) {
  const realm = await pendingRealm(root);
  await act(async () => {
    realm.iframe.dispatchEvent(new Event('load'));
    realm.channel.port2.postMessage({ type: 'ready' });
    realm.channel.port2.postMessage({ type: 'mounted' });
  });
  return realm;
}

async function failRealm(channel: FakeMessageChannel, message: string) {
  await act(async () => {
    channel.port2.postMessage({ type: 'failed', message });
  });
}

describe('isolated frontend React adapter callback and realm ownership', () => {
  beforeEach(() => {
    vi.stubGlobal('MessageChannel', FakeMessageChannel);
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    document.body.replaceChildren();
    FakeMessageChannel.instances.length = 0;
  });

  test('uses the latest error callback and props inside the same mounted realm', async () => {
    const previous = vi.fn();
    const latest = vi.fn();
    const view = setup(previous);
    const { iframe, channel } = await mountRealm(view.root);
    const latestProps = { label: 'latest', count: 2 };

    view.rerender(
      <FrontstageIsolatedFrontendBlockHost
        root={view.root}
        preparation={preparation(latestProps)}
        onRuntimeError={latest}
      />
    );

    expect(view.root.querySelector('iframe')).toBe(iframe);
    expect(FakeMessageChannel.instances).toHaveLength(1);
    expect(channel.port1.closed).toBe(false);
    expect(channel.port2.closed).toBe(false);
    expect(channel.port1.sent).toContainEqual({
      type: 'update',
      props: latestProps
    });

    await failRealm(channel, 'latest callback failure');

    expect(latest).toHaveBeenCalledExactlyOnceWith(
      new Error('latest callback failure')
    );
    expect(previous).not.toHaveBeenCalled();
  });

  test('stops calling a removed callback without replacing the realm', async () => {
    const previous = vi.fn();
    const view = setup(previous);
    const { iframe, channel } = await mountRealm(view.root);

    view.rerender(
      <FrontstageIsolatedFrontendBlockHost
        root={view.root}
        preparation={view.initial}
      />
    );

    expect(view.root.querySelector('iframe')).toBe(iframe);
    expect(FakeMessageChannel.instances).toHaveLength(1);

    await failRealm(channel, 'callback removed');

    expect(previous).not.toHaveBeenCalled();
    expect(view.root).toBeEmptyDOMElement();
    expect(channel.port1.closed).toBe(true);
    expect(channel.port2.closed).toBe(true);
  });

  test('keeps the latest committed callback and props when mounting finishes later', async () => {
    const previous = vi.fn();
    const latest = vi.fn();
    const view = setup(previous);
    const { iframe, channel } = await pendingRealm(view.root);
    const latestProps = { label: 'changed while mounting' };

    view.rerender(
      <FrontstageIsolatedFrontendBlockHost
        root={view.root}
        preparation={preparation(latestProps)}
        onRuntimeError={latest}
      />
    );
    expect(view.root.querySelector('iframe')).toBe(iframe);
    expect(FakeMessageChannel.instances).toHaveLength(1);

    await act(async () => {
      iframe.dispatchEvent(new Event('load'));
      channel.port2.postMessage({ type: 'ready' });
      channel.port2.postMessage({ type: 'mounted' });
    });

    expect(channel.port1.sent).toEqual([
      { type: 'mount', props: view.initial.program.props },
      { type: 'update', props: latestProps }
    ]);
    await failRealm(channel, 'failure after delayed mount');
    expect(latest).toHaveBeenCalledExactlyOnceWith(
      new Error('failure after delayed mount')
    );
    expect(previous).not.toHaveBeenCalled();
  });

  test('updates props over the existing port without remounting', async () => {
    const view = setup();
    const { iframe, channel } = await mountRealm(view.root);
    const props = { label: 'props only', values: [0, false, null] };

    view.rerender(
      <FrontstageIsolatedFrontendBlockHost
        root={view.root}
        preparation={preparation(props)}
        onRuntimeError={view.onRuntimeError}
      />
    );

    expect(view.root.querySelector('iframe')).toBe(iframe);
    expect(FakeMessageChannel.instances).toHaveLength(1);
    expect(channel.port1.sent).toEqual([
      { type: 'mount', props: view.initial.program.props },
      { type: 'update', props }
    ]);
    expect(channel.port1.closed).toBe(false);
    expect(channel.port2.closed).toBe(false);
  });

  test.each(['source', 'contribution identity'] as const)(
    'replaces the realm when its %s changes and ignores the old transport',
    async (change) => {
      const previous = vi.fn();
      const latest = vi.fn();
      const view = setup(previous);
      const original = await mountRealm(view.root);
      const next = preparation(
        { label: 'replacement' },
        change === 'contribution identity'
          ? 'another-contribution'
          : view.initial.contributionId,
        change === 'source'
          ? `${PROGRAM_SOURCE}\n// source revision two`
          : PROGRAM_SOURCE
      );

      view.rerender(
        <FrontstageIsolatedFrontendBlockHost
          root={view.root}
          preparation={next}
          onRuntimeError={latest}
        />
      );
      await waitFor(() => expect(FakeMessageChannel.instances).toHaveLength(2));
      const replacement = await mountRealm(view.root);

      expect(replacement.iframe).not.toBe(original.iframe);
      expect(replacement.channel).not.toBe(original.channel);
      expect(original.iframe.isConnected).toBe(false);
      expect(original.channel.port1.closed).toBe(true);
      expect(original.channel.port2.closed).toBe(true);
      expect(original.channel.port1.sent).toContainEqual({ type: 'terminate' });
      expect(replacement.channel.port1.sent).toContainEqual({
        type: 'mount',
        props: next.program.props
      });

      await failRealm(original.channel, 'old realm failure');
      expect(previous).not.toHaveBeenCalled();
      expect(latest).not.toHaveBeenCalled();
      await failRealm(replacement.channel, 'replacement failure');
      expect(latest).toHaveBeenCalledExactlyOnceWith(
        new Error('replacement failure')
      );
      expect(previous).not.toHaveBeenCalled();
    }
  );

  test('terminates the current realm on unmount and ignores late errors', async () => {
    const view = setup();
    const { iframe, channel } = await mountRealm(view.root);

    view.unmount();

    expect(view.root).toBeEmptyDOMElement();
    expect(iframe.isConnected).toBe(false);
    expect(channel.port1.closed).toBe(true);
    expect(channel.port2.closed).toBe(true);
    expect(channel.port1.sent).toContainEqual({ type: 'terminate' });
    await failRealm(channel, 'late failure after unmount');
    expect(view.onRuntimeError).not.toHaveBeenCalled();
  });
});
