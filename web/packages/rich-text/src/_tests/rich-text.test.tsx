import { act, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const destroy = vi.fn();
const setValue = vi.fn();
const getValue = vi.fn(() => 'initial');
const options: Array<Record<string, unknown>> = [];

vi.mock('vditor/dist/js/lute/lute.min.js', () => ({}));
vi.mock('vditor/dist/js/i18n/zh_CN.js', () => ({}));
vi.mock('vditor/dist/js/icons/ant.js', () => ({}));
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
    setValue = setValue;
  }
}));

import { MarkdownEditor, MarkdownPreview } from '../index';

describe('@1flowbase/rich-text (AC-PUB-006)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    options.length = 0;
    document.getElementById('vditorLuteScript')?.remove();
    document.getElementById('vditorIconScript')?.remove();
  });

  it('owns a controlled editor with cache, uploads and remote CDN disabled', async () => {
    const view = render(
      <MarkdownEditor value="initial" onChange={vi.fn()} ariaLabel="editor" />
    );
    await act(async () => undefined);

    expect(options[0]).toMatchObject({
      cache: { enable: false },
      cdn: '/__1flowbase_bundled_vditor__',
      upload: { linkToImgUrl: '', url: '' }
    });
    expect(options[0]?.toolbar).not.toContain('upload');

    view.unmount();
    expect(destroy).toHaveBeenCalledTimes(1);
  });

  it('sanitizes preview HTML and removes network-capable content', async () => {
    render(<MarkdownPreview aria-label="preview" value="fixture" />);
    await act(async () => undefined);

    const preview = screen.getByLabelText('preview');
    expect(preview.querySelector('img')).toBeNull();
    expect(preview.querySelector('a')).not.toHaveAttribute('href');
  });

  it('owns two independent instances and releases shared support markers after the last unmount', async () => {
    const first = render(
      <MarkdownEditor value="first" onChange={vi.fn()} ariaLabel="first" />
    );
    const second = render(
      <MarkdownEditor value="second" onChange={vi.fn()} ariaLabel="second" />
    );
    await act(async () => undefined);

    expect(options).toHaveLength(2);
    expect(document.getElementById('vditorLuteScript')).not.toBeNull();
    first.unmount();
    expect(destroy).toHaveBeenCalledTimes(1);
    expect(document.getElementById('vditorLuteScript')).not.toBeNull();
    second.unmount();
    expect(destroy).toHaveBeenCalledTimes(2);
    expect(document.getElementById('vditorLuteScript')).toBeNull();
    expect(document.getElementById('vditorIconScript')).toBeNull();
  });
});
