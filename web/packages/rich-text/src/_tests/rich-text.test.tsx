import { act, render } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const destroy = vi.fn();
const setValue = vi.fn();
const getValue = vi.fn(() => 'initial');
const getHTML = vi.fn(() => '<p>initial</p>');
const insertValue = vi.fn();
const focus = vi.fn();
const blur = vi.fn();
const setPreviewMode = vi.fn();
const options: Array<Record<string, unknown>> = [];

vi.mock('vditor/dist/js/lute/lute.min.js', () => ({}));
vi.mock('vditor/dist/js/i18n/zh_CN.js', () => ({}));
vi.mock('vditor/dist/js/icons/ant.js?raw', () => ({
  default:
    "document.body.insertAdjacentHTML('afterbegin', `<svg xmlns=\"http://www.w3.org/2000/svg\"><defs><symbol id=\"vditor-icon-headings\" viewBox=\"0 0 32 32\"><path d=\"M0 0h1v1H0z\"></path></symbol></defs></svg>`)"
}));
vi.mock('vditor', () => ({
  default: class VditorFixture {
    static md2html = vi.fn(
      async () =>
        '<p>safe</p><img src="https://remote.test/a.png"><a href="https://remote.test">remote</a>'
    );
    constructor(_mount: HTMLElement, nextOptions: Record<string, unknown>) {
      options.push(nextOptions);
    }
    destroy = destroy;
    getValue = getValue;
    getHTML = getHTML;
    insertValue = insertValue;
    focus = focus;
    blur = blur;
    setPreviewMode = setPreviewMode;
    setValue = setValue;
  }
}));

import { VditorEditor } from '../index';

describe('@1flowbase/rich-text unified Vditor contract', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    options.length = 0;
  });

  it('AC-002 owns one full editor with local assets and the native preview experience', async () => {
    const view = render(
      <VditorEditor value="initial" onChange={vi.fn()} ariaLabel="editor" />
    );
    await act(async () => undefined);

    expect(options[0]).toMatchObject({
      cache: { enable: false },
      cdn: '/__1flowbase_bundled_vditor__',
      mode: 'ir',
      preview: { mode: 'both' }
    });
    expect(options[0]?.toolbar).toContain('fullscreen');
    expect(options[0]?.toolbar).toContain('edit-mode');

    view.unmount();
    expect(destroy).toHaveBeenCalledTimes(1);
  });

  it('AC-003 uploads through the governed block API and inserts the platform content URL', async () => {
    const post = vi.fn(async () => ({
      file_table_id: 'table-1',
      record: { id: 'record-1' },
      storage_id: 'storage-1'
    }));
    const view = render(
      <VditorEditor
        api={{ post }}
        ariaLabel="editor"
        value="initial"
        onChange={vi.fn()}
      />
    );
    await act(async () => undefined);

    const upload = options[0]?.upload as {
      handler(files: File[]): Promise<string | null>;
    };
    const file = {
      name: 'diagram.png',
      type: 'image/png',
      arrayBuffer: async () => new Uint8Array([1, 2, 3]).buffer
    } as File;
    await expect(upload.handler([file])).resolves.toBeNull();

    expect(post).toHaveBeenCalledWith('/api/console/files/upload', {
      body: {
        file: {
          base64: 'AQID',
          content_type: 'image/png',
          file_name: 'diagram.png'
        }
      }
    });
    expect(insertValue).toHaveBeenCalledWith(
      '![diagram.png](/api/console/files/table-1/records/record-1/content)\n'
    );
    view.unmount();
  });

  it('owns two independent instances and releases shared support markers after the last unmount', async () => {
    const { unmount: unmountFirst } = render(
      <VditorEditor value="first" onChange={vi.fn()} ariaLabel="first" />
    );
    const { unmount: unmountSecond } = render(
      <VditorEditor value="second" onChange={vi.fn()} ariaLabel="second" />
    );
    await act(async () => undefined);

    expect(options).toHaveLength(2);
    expect(document.getElementById('vditorLuteScript')).not.toBeNull();
    expect(document.getElementById('vditor-icon-headings')).not.toBeNull();
    unmountFirst();
    expect(destroy).toHaveBeenCalledTimes(1);
    expect(document.getElementById('vditorLuteScript')).not.toBeNull();
    expect(document.getElementById('vditor-icon-headings')).not.toBeNull();
    unmountSecond();
    expect(destroy).toHaveBeenCalledTimes(2);
    expect(document.getElementById('vditorLuteScript')).toBeNull();
    expect(document.getElementById('vditorIconScript')).toBeNull();
    expect(document.getElementById('vditor-icon-headings')).toBeNull();
  });

  it('AC-SHADOW-001 provides one icon sprite per ShadowRoot until its last editor unmounts', async () => {
    const host = document.createElement('div');
    const shadowRoot = host.attachShadow({ mode: 'open' });
    const firstContainer = document.createElement('div');
    const secondContainer = document.createElement('div');
    shadowRoot.append(firstContainer, secondContainer);
    document.body.append(host);

    const { unmount: unmountFirst } = render(
      <VditorEditor value="first" onChange={vi.fn()} ariaLabel="first" />,
      { container: firstContainer }
    );
    const { unmount: unmountSecond } = render(
      <VditorEditor value="second" onChange={vi.fn()} ariaLabel="second" />,
      { container: secondContainer }
    );
    await act(async () => undefined);

    expect(
      shadowRoot.querySelectorAll('[data-1flowbase-vditor-icons]')
    ).toHaveLength(1);
    expect(shadowRoot.getElementById('vditor-icon-headings')).not.toBeNull();

    unmountFirst();
    expect(shadowRoot.getElementById('vditor-icon-headings')).not.toBeNull();
    unmountSecond();
    expect(shadowRoot.getElementById('vditor-icon-headings')).toBeNull();
    host.remove();
  });
});
// @vitest-environment jsdom

import '@testing-library/jest-dom/vitest';
