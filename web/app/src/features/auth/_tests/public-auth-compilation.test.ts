import { afterEach, describe, expect, test, vi } from 'vitest';
import { compileNativeReactComponent } from '@1flowbase/page-runtime';
import { createPublicAuthCompilation } from '../lib/public-auth-compilation';
import type { NativeReactBrowserCompileResult } from '../../../shared/code-block/native-react-compiler-browser';

const source = 'export default function Auth() { return null; }';
const request = { source, requestId: 'first', moduleDefinitions: [] };
const result = () => compileNativeReactComponent(source);
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
afterEach(() => vi.useRealTimers());
describe('I2012 public auth compilation budget', () => {
  test('AC-002 shares in-flight work and reuses only matching source and module contracts', async () => {
    const work = deferred<NativeReactBrowserCompileResult>();
    const compiler = vi
      .fn()
      .mockReturnValueOnce(work.promise)
      .mockImplementation(async ({ source, moduleDefinitions }) =>
        compileNativeReactComponent(source, moduleDefinitions)
      );
    const compilation = createPublicAuthCompilation(compiler);
    const first = compilation.compile(request);
    const second = compilation.compile({ ...request, requestId: 'second' });
    work.resolve(result());
    expect(await first).toMatchObject({ ok: true });
    expect(await second).toEqual(await first);
    expect(await compilation.compile(request)).toEqual(await first);
    expect(compiler).toHaveBeenCalledTimes(1);
    await compilation.compile({ ...request, source: source + ' ' });
    await compilation.compile({
      ...request,
      moduleDefinitions: [{ module_source: 'react', exports: ['useState'] }]
    });
    expect(compiler).toHaveBeenCalledTimes(3);
  });
  test('AC-001/002 prefetch compiles without evaluating user code and yields queued work to demand', async () => {
    const work = deferred<NativeReactBrowserCompileResult>();
    const order: string[] = [];
    const compiler = vi.fn().mockImplementation(({ requestId }) => {
      order.push(requestId);
      return order.length === 1 ? work.promise : Promise.resolve(result());
    });
    const compilation = createPublicAuthCompilation(compiler);
    const first = compilation.prefetch(request);
    const background = compilation.prefetch({
      ...request,
      source: source + ' ',
      requestId: 'background'
    });
    const foreground = compilation.compile({
      ...request,
      source: source + '\n',
      requestId: 'foreground'
    });
    expect(order).toEqual(['first']);
    work.resolve(result());
    await Promise.all([first, background, foreground]);
    expect(order).toEqual(['first', 'foreground', 'background']);
    const unsafe =
      "throw new Error('must not evaluate'); export default function Auth() { return null; }";
    const real = createPublicAuthCompilation(
      async ({ source, moduleDefinitions }) =>
        compileNativeReactComponent(source, moduleDefinitions)
    );
    expect(await real.prefetch({ ...request, source: unsafe })).toMatchObject({
      ok: true
    });
  });
  test('AC-002 times out and cancels stalled work, then permits a fresh attempt', async () => {
    vi.useFakeTimers();
    const compiler = vi
      .fn()
      .mockImplementationOnce(() => new Promise(() => {}))
      .mockResolvedValue(result());
    const compilation = createPublicAuthCompilation(compiler);
    const first = compilation.compile(request);
    await vi.advanceTimersByTimeAsync(9000);
    expect(await first).toMatchObject({ ok: false });
    expect(compiler.mock.calls[0][0].signal.aborted).toBe(true);
    expect(await compilation.compile(request)).toMatchObject({ ok: true });
  });
  test('AC-002 bounds speculative and foreground queues and promotes matching intent', async () => {
    const work = deferred<NativeReactBrowserCompileResult>();
    const order: string[] = [];
    const compiler = vi.fn().mockImplementation(({ requestId }) => {
      order.push(requestId);
      return order.length === 1 ? work.promise : Promise.resolve(result());
    });
    const compilation = createPublicAuthCompilation(compiler);
    const active = compilation.compile(request);
    const oldIntent = compilation.prefetch({
      ...request,
      requestId: 'old',
      source: source + 'a'
    });
    const intent = compilation.prefetch({
      ...request,
      requestId: 'intent',
      source: source + 'b'
    });
    expect(await oldIntent).toMatchObject({ ok: false });
    const promoted = compilation.compile({
      ...request,
      requestId: 'selected',
      source: source + 'b'
    });
    expect(promoted).toBe(intent);
    const third = compilation.compile({
      ...request,
      requestId: 'third',
      source: source + 'c'
    });
    const fourth = compilation.compile({
      ...request,
      requestId: 'fourth',
      source: source + 'd'
    });
    expect(
      await compilation.compile({ ...request, source: source + 'e' })
    ).toMatchObject({ ok: false });
    work.resolve(result());
    await Promise.all([active, intent, promoted, third, fourth]);
    expect(order).toEqual(['first', 'intent', 'third', 'fourth']);
  });
  test('AC-002 does not retain an artifact larger than the byte budget', async () => {
    const source = `const text = '${'x'.repeat(1100000)}'; export default function Auth() { return text; }`;
    const compiled = compileNativeReactComponent(source);
    if (!compiled.ok)
      throw new Error('Oversized artifact fixture did not compile.');
    expect(
      new TextEncoder().encode(JSON.stringify(compiled.artifact)).byteLength
    ).toBeGreaterThan(1024 * 1024);
    const compiler = vi.fn().mockResolvedValue(compiled);
    const compilation = createPublicAuthCompilation(compiler);
    expect(await compilation.compile({ ...request, source })).toMatchObject({
      ok: true
    });
    expect(await compilation.compile({ ...request, source })).toMatchObject({
      ok: true
    });
    expect(compiler).toHaveBeenCalledTimes(2);
  });

  test('AC-002 evicts old artifacts and never caches failures', async () => {
    const compiler = vi
      .fn()
      .mockImplementation(async ({ source, moduleDefinitions }) =>
        compileNativeReactComponent(source, moduleDefinitions)
      );
    const compilation = createPublicAuthCompilation(compiler);
    for (let i = 0; i < 6; i++)
      await compilation.compile({ ...request, source: source + ' '.repeat(i) });
    await compilation.compile(request);
    expect(compiler).toHaveBeenCalledTimes(7);
    await compilation.compile({ ...request, source: 'invalid !!!' });
    await compilation.compile({ ...request, source: 'invalid !!!' });
    expect(compiler).toHaveBeenCalledTimes(9);
  });
});
