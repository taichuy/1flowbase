import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { VditorEditor, VditorPreview } from '../index';

const { instances, load, preview } = vi.hoisted(() => ({
  instances: [] as Array<{
    mount: HTMLElement;
    options: IOptions;
    value: string;
    destroy: ReturnType<typeof vi.fn>;
    setValue: ReturnType<typeof vi.fn>;
    insertValue: ReturnType<typeof vi.fn>;
  }>,
  load: vi.fn(),
  preview: vi.fn()
}));
vi.mock('../runtime-assets', async (original) => {
  const actual = await original<typeof import('../runtime-assets')>();
  class Editor {
    value: string;
    destroy = vi.fn();
    setValue = vi.fn((value: string) => {
      this.value = value;
    });
    insertValue = vi.fn();
    getValue = () => this.value;
    getHTML = () => '<p>draft</p>';
    focus = vi.fn();
    blur = vi.fn();
    setPreviewMode = vi.fn();
    constructor(
      public mount: HTMLElement,
      public options: IOptions
    ) {
      this.value = options.value ?? '';
      instances.push(this);
      queueMicrotask(() => options.after?.());
    }
    static preview = preview;
  }
  return {
    ...actual,
    loadFrameRuntime: load.mockImplementation(async () => Editor)
  };
});

async function mountFrame(title = 'editor') {
  const frame = screen.getByTitle(title) as HTMLIFrameElement;
  // jsdom does not fetch the frame's static HTML resource.
  if (!frame.contentDocument?.body)
    Object.defineProperty(frame, 'contentDocument', {
      configurable: true,
      value: document.implementation.createHTMLDocument('frame')
    });
  await act(async () => fireEvent.load(frame));
  await waitFor(() => expect(load).toHaveBeenCalled());
  return frame;
}

beforeEach(() => {
  instances.length = 0;
  load.mockClear();
  preview.mockReset();
});

describe('Vditor frame lifecycle', () => {
  it('shows loading until initialization finishes and replaces it on failure', async () => {
    let reject!: (error: Error) => void;
    load.mockImplementationOnce(
      () =>
        new Promise((_, fail) => {
          reject = fail;
        })
    );
    const view = render(
      <VditorEditor value="" onChange={vi.fn()} ariaLabel="editor" />
    );
    expect(screen.getByRole('status')).toHaveTextContent('编辑器加载中');
    await mountFrame();
    expect(screen.getByRole('status')).toBeInTheDocument();
    await act(async () => reject(new Error('Resource unavailable')));
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('加载失败');
    view.unmount();
  });

  it('removes loading only when the editor is ready', async () => {
    const view = render(
      <VditorEditor value="" onChange={vi.fn()} ariaLabel="editor" />
    );
    expect(screen.getByRole('status')).toBeInTheDocument();
    await mountFrame();
    await waitFor(() =>
      expect(screen.queryByRole('status')).not.toBeInTheDocument()
    );
    view.unmount();
  });

  it('keeps editor selections in its own document, including when hosted by a ShadowRoot', async () => {
    const host = document.createElement('div');
    const shadow = host.attachShadow({ mode: 'open' });
    document.body.appendChild(host);
    const ready = vi.fn();
    const view = render(
      <VditorEditor
        value="draft"
        onChange={vi.fn()}
        onReady={ready}
        ariaLabel="editor"
      />,
      { container: shadow }
    );
    const frame = shadow.querySelector('iframe')!;
    // jsdom does not create browsing contexts for frames inside a ShadowRoot.
    const browsingContext = document.createElement('iframe');
    document.body.appendChild(browsingContext);
    Object.defineProperty(frame, 'contentDocument', {
      value: browsingContext.contentDocument
    });
    await act(async () => fireEvent.load(frame));
    await waitFor(() =>
      expect(ready).toHaveBeenCalledWith(
        expect.objectContaining({ getValue: expect.any(Function) })
      )
    );
    expect(instances[0].mount.ownerDocument).toBe(frame.contentDocument);
    expect(instances[0].mount.getRootNode()).toBe(frame.contentDocument);
    expect(instances[0].options).toMatchObject({
      mode: 'ir',
      typewriterMode: true,
      cache: { enable: false },
      preview: {
        hljs: { enable: true },
        markdown: {
          sanitize: false,
          codeBlockPreview: true,
          mathBlockPreview: true
        }
      }
    });
    view.unmount();
    expect(instances[0].destroy).toHaveBeenCalledOnce();
    expect(ready).toHaveBeenLastCalledWith(null);
    host.remove();
    browsingContext.remove();
  });

  it('preserves an unreported draft across a locale rebuild and ignores disposed input', async () => {
    const changed = vi.fn();
    const view = render(
      <VditorEditor value="initial" onChange={changed} ariaLabel="editor" />
    );
    await mountFrame();
    const first = instances[0];
    first.value = 'unreported draft';
    view.rerender(
      <VditorEditor
        value="initial"
        onChange={changed}
        ariaLabel="editor"
        locale="en_US"
      />
    );
    await waitFor(() => expect(instances).toHaveLength(2));
    expect(instances[1].options.value).toBe('unreported draft');
    expect(changed).toHaveBeenCalledWith('unreported draft');
    changed.mockClear();
    act(() => first.options.input?.('stale', undefined as never));
    expect(changed).not.toHaveBeenCalled();
    view.unmount();
  });

  it('updates external content without recreating the editor', async () => {
    const changed = vi.fn();
    const view = render(
      <VditorEditor value="first" onChange={changed} ariaLabel="editor" />
    );
    await mountFrame();
    view.rerender(
      <VditorEditor value="second" onChange={changed} ariaLabel="editor" />
    );
    expect(instances).toHaveLength(1);
    expect(instances[0].setValue).toHaveBeenCalledWith('second', true);
    view.unmount();
  });

  it('keeps uploads on the existing governed API and escapes file labels', async () => {
    const post = vi.fn(async () => ({
      file_table_id: 'table',
      record: { id: 'record' },
      storage_id: 'storage'
    }));
    const view = render(
      <VditorEditor
        api={{ post }}
        value=""
        onChange={vi.fn()}
        ariaLabel="editor"
      />
    );
    await mountFrame();
    const file = {
      name: 'a[b].png',
      type: 'image/png',
      arrayBuffer: async () => new Uint8Array([1, 2, 3]).buffer
    } as File;
    await instances[0].options.upload?.handler?.([file]);
    expect(post).toHaveBeenCalledWith('/api/console/files/upload', {
      body: {
        file: {
          base64: 'AQID',
          content_type: 'image/png',
          file_name: 'a[b].png'
        }
      }
    });
    expect(instances[0].insertValue).toHaveBeenCalledWith(
      '![a\\[b\\].png](/api/console/files/table/records/record/content)\n'
    );
    view.unmount();
  });

  it('does not construct an editor after its frame was unmounted during loading', async () => {
    let resolve!: (runtime: unknown) => void;
    load.mockImplementationOnce(
      () =>
        new Promise((r) => {
          resolve = r;
        })
    );
    const view = render(
      <VditorEditor value="draft" onChange={vi.fn()} ariaLabel="editor" />
    );
    await mountFrame();
    view.unmount();
    const constructor = vi.fn();
    await act(async () => resolve(constructor));
    expect(constructor).not.toHaveBeenCalled();
  });

  it('ignores a stale asynchronous read render after the article changes', async () => {
    const pending: Array<() => void> = [];
    preview.mockImplementation(
      (element: HTMLElement, value: string) =>
        new Promise<void>((resolve) => {
          element.textContent = value;
          pending.push(resolve);
        })
    );
    const view = render(<VditorPreview value="old" ariaLabel="reader" />);
    const frame = await mountFrame('reader');
    expect(screen.getByRole('status')).toHaveTextContent('正文加载中');
    view.rerender(<VditorPreview value="new" ariaLabel="reader" />);
    await waitFor(() => expect(pending).toHaveLength(2));
    await act(async () => pending[0]());
    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(frame.contentDocument?.body).not.toHaveTextContent('old');
    view.unmount();
    await act(async () => pending[1]());
    expect(frame.contentDocument?.body).not.toHaveTextContent('new');
  });
});
