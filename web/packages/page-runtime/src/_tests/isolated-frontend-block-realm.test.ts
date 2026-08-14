// @vitest-environment jsdom

import '@testing-library/jest-dom/vitest';

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import {
  ISOLATED_FRONTEND_BLOCK_SANDBOX,
  createIsolatedFrontendBlockSrcdoc,
  prepareIsolatedFrontendBlockRealm,
  validateIsolatedFrontendBlockSource
} from '../isolated-frontend-block-realm';

const PROGRAM_SOURCE = `globalThis.__oneflowbaseIsolatedBlock = {
  mount(root, props) {
    root.textContent = String(props.label);
    return { update(next) { root.textContent = String(next.label); } };
  }
};`;

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
  static latest: FakeMessageChannel | null = null;
  readonly port1 = new FakeMessagePort();
  readonly port2 = new FakeMessagePort();

  constructor() {
    this.port1.peer = this.port2;
    this.port2.peer = this.port1;
    FakeMessageChannel.latest = this;
  }
}

describe('isolated frontend block realm', () => {
  beforeEach(() => {
    vi.stubGlobal('MessageChannel', FakeMessageChannel);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    document.body.replaceChildren();
    FakeMessageChannel.latest = null;
  });

  test('D5-P3 creates a hash-locked opaque iframe document with default-deny CSP', async () => {
    const srcdoc = await createIsolatedFrontendBlockSrcdoc(PROGRAM_SOURCE);

    expect(ISOLATED_FRONTEND_BLOCK_SANDBOX).toBe('allow-scripts');
    expect(ISOLATED_FRONTEND_BLOCK_SANDBOX).not.toContain('allow-same-origin');
    expect(srcdoc).toContain("default-src 'none'");
    expect(srcdoc).toContain("connect-src 'none'");
    expect(srcdoc).toContain("object-src 'none'");
    expect(srcdoc).toContain("base-uri 'none'");
    expect(srcdoc).toContain("form-action 'none'");
    expect(srcdoc).toContain("frame-src 'none'");
    expect(srcdoc).toContain("worker-src 'none'");
    expect(srcdoc).toContain("child-src 'none'");
    expect(srcdoc).toMatch(/script-src 'sha256-[^']+' 'sha256-[^']+'/u);
    expect(srcdoc).not.toContain('allow-same-origin');
    expect(srcdoc).not.toMatch(/\beval\s*\(|new Function/u);
  });

  test.each([
    "import/* comment */('https://example.test/module.js')",
    'export const value = 1',
    "require('package')",
    "importScripts('worker.js')",
    '</script><script>alert(1)</script>'
  ])(
    'rejects source outside the self-contained realm contract: %s',
    (source) => {
      expect(() => validateIsolatedFrontendBlockSource(source)).toThrow(
        'without imports or exports'
      );
    }
  );

  test('mounts, updates, defaults unknown capabilities to deny, and terminates idempotently', async () => {
    const root = document.createElement('div');
    document.body.append(root);
    const publish = vi.fn(() => ({ accepted: true }));
    const realm = prepareIsolatedFrontendBlockRealm(
      { source: PROGRAM_SOURCE, props: { label: 'first' } },
      { capabilityHandlers: { 'block.output.publish': publish } }
    );

    const mounting = realm.mount(root);
    await nextTask();
    const iframe = root.querySelector('iframe');
    expect(iframe).not.toBeNull();
    expect(iframe).toHaveAttribute('sandbox', 'allow-scripts');
    expect(iframe).toHaveAttribute('referrerpolicy', 'no-referrer');
    iframe?.dispatchEvent(new Event('load'));
    const channel = FakeMessageChannel.latest!;
    channel.port2.postMessage({ type: 'ready' });
    expect(channel.port1.sent).toContainEqual({
      type: 'mount',
      props: { label: 'first' }
    });
    channel.port2.postMessage({ type: 'mounted' });
    await mounting;
    expect(realm.state).toBe('mounted');

    channel.port2.postMessage({
      type: 'capability_request',
      requestId: 'denied',
      capability: 'host.fetch',
      payload: null
    });
    expect(channel.port1.sent).toContainEqual({
      type: 'capability_response',
      requestId: 'denied',
      ok: false,
      error: 'capability_denied'
    });
    expect(publish).not.toHaveBeenCalled();

    channel.port2.postMessage({
      type: 'capability_request',
      requestId: 'published',
      capability: 'block.output.publish',
      payload: { port: 'result', value: 42 }
    });
    await Promise.resolve();
    expect(publish).toHaveBeenCalledWith({ port: 'result', value: 42 });
    expect(channel.port1.sent).toContainEqual({
      type: 'capability_response',
      requestId: 'published',
      ok: true,
      value: { accepted: true }
    });

    realm.update({ label: 'second' });
    expect(realm.state).toBe('updated');
    expect(channel.port1.sent).toContainEqual({
      type: 'update',
      props: { label: 'second' }
    });

    realm.terminate();
    realm.dispose();
    expect(realm.state).toBe('terminated');
    expect(root).toBeEmptyDOMElement();
    expect(channel.port1.closed).toBe(true);
    expect(channel.port2.closed).toBe(true);
    const sentCount = channel.port1.sent.length;
    channel.port2.postMessage({ type: 'failed', message: 'late failure' });
    expect(channel.port1.sent).toHaveLength(sentCount);
  });

  test('cleans up mount failures and forwards failures that arrive after mount', async () => {
    const root = document.createElement('div');
    document.body.append(root);
    const onError = vi.fn();
    const realm = prepareIsolatedFrontendBlockRealm(
      { source: PROGRAM_SOURCE, props: {} },
      { onError }
    );

    const mounting = realm.mount(root);
    await nextTask();
    root.querySelector('iframe')?.dispatchEvent(new Event('load'));
    const channel = FakeMessageChannel.latest!;
    channel.port2.postMessage({ type: 'ready' });
    channel.port2.postMessage({ type: 'mounted' });
    await mounting;
    channel.port2.postMessage({ type: 'failed', message: 'runtime failed' });

    expect(realm.state).toBe('failed');
    expect(onError).toHaveBeenCalledWith(new Error('runtime failed'));
    expect(root).toBeEmptyDOMElement();
    expect(channel.port1.closed).toBe(true);
    realm.terminate();
  });

  test('rejects a failed mount and releases its iframe, ports, and listeners', async () => {
    const root = document.createElement('div');
    document.body.append(root);
    const onError = vi.fn();
    const realm = prepareIsolatedFrontendBlockRealm(
      { source: PROGRAM_SOURCE, props: {} },
      { onError }
    );

    const mounting = realm.mount(root);
    await nextTask();
    root.querySelector('iframe')?.dispatchEvent(new Event('load'));
    const channel = FakeMessageChannel.latest!;
    channel.port2.postMessage({ type: 'ready' });
    channel.port2.postMessage({ type: 'failed', message: 'mount failed' });

    await expect(mounting).rejects.toThrow('mount failed');
    expect(realm.state).toBe('failed');
    expect(onError).toHaveBeenCalledWith(new Error('mount failed'));
    expect(root).toBeEmptyDOMElement();
    expect(channel.port1.closed).toBe(true);
    expect(channel.port2.closed).toBe(true);
  });
});

async function nextTask(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}
