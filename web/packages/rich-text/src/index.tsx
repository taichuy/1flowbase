import { useEffect, useRef } from 'react';
import type { HTMLAttributes } from 'react';

import Vditor from 'vditor';
import 'vditor/dist/index.css';

import { markBundledVditorRuntime } from './runtime-assets';
import './styles.css';

const NO_REMOTE_ASSET_BASE = '/__1flowbase_bundled_vditor__';

export interface MarkdownEditorProps {
  readonly ariaLabel?: string;
  readonly className?: string;
  readonly height?: number | string;
  readonly placeholder?: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
}

export function MarkdownEditor({
  ariaLabel = 'markdown_editor',
  className,
  height = 180,
  placeholder = '',
  value,
  onChange
}: MarkdownEditorProps) {
  const mountRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<Vditor | null>(null);
  const readyRef = useRef(false);
  const valueRef = useRef(value);
  const onChangeRef = useRef(onChange);

  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  useEffect(() => {
    valueRef.current = value;
    const editor = editorRef.current;
    if (editor && readyRef.current && editor.getValue() !== value) {
      editor.setValue(value, true);
    }
  }, [value]);

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return undefined;

    let disposed = false;
    queueMicrotask(() => {
      if (disposed) return;
      markBundledVditorRuntime();
      const editor = new Vditor(mount, {
        _lutePath: NO_REMOTE_ASSET_BASE,
        cache: { enable: false },
        cdn: NO_REMOTE_ASSET_BASE,
        height,
        hint: { emoji: {}, emojiPath: NO_REMOTE_ASSET_BASE },
        i18n: window.VditorI18n,
        image: { isPreview: false },
        mode: 'ir',
        placeholder,
        preview: {
          hljs: { enable: false },
          markdown: {
            codeBlockPreview: false,
            mathBlockPreview: false,
            sanitize: true
          },
          mode: 'editor',
          render: { media: { enable: false } },
          theme: { current: 'ant-design', path: NO_REMOTE_ASSET_BASE }
        },
        toolbar: [
          'headings',
          'bold',
          'italic',
          'strike',
          '|',
          'list',
          'ordered-list',
          'check',
          '|',
          'quote',
          'code',
          'link',
          'table',
          '|',
          'undo',
          'redo'
        ],
        upload: { url: '', linkToImgUrl: '' },
        value: valueRef.current,
        after: () => {
          if (disposed) {
            disposeEditor(editor);
            return;
          }
          readyRef.current = true;
          if (editor.getValue() !== valueRef.current) {
            editor.setValue(valueRef.current, true);
          }
        },
        input: (nextValue) => {
          valueRef.current = nextValue;
          onChangeRef.current(nextValue);
        }
      });
      editorRef.current = editor;
    });

    return () => {
      disposed = true;
      readyRef.current = false;
      const editor = editorRef.current;
      editorRef.current = null;
      if (editor) disposeEditor(editor);
    };
  }, [height, placeholder]);

  return (
    <div
      ref={mountRef}
      aria-label={ariaLabel}
      className={joinClassNames('oneflow-markdown-editor', className)}
    />
  );
}

export interface MarkdownPreviewProps extends Omit<
  HTMLAttributes<HTMLDivElement>,
  'children'
> {
  readonly value: string;
}

export function MarkdownPreview({
  className,
  value,
  ...previewProps
}: MarkdownPreviewProps) {
  const mountRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return undefined;
    let disposed = false;
    markBundledVditorRuntime();

    void Vditor.md2html(value, {
      cdn: NO_REMOTE_ASSET_BASE,
      emojiPath: NO_REMOTE_ASSET_BASE,
      hljs: { enable: false },
      i18n: window.VditorI18n,
      markdown: {
        codeBlockPreview: false,
        mathBlockPreview: false,
        sanitize: true
      },
      mode: 'light',
      render: { media: { enable: false } }
    }).then((html) => {
      if (!disposed) mount.replaceChildren(sanitizePreview(html));
    });

    return () => {
      disposed = true;
      mount.replaceChildren();
    };
  }, [value]);

  return (
    <div
      ref={mountRef}
      className={joinClassNames(
        'oneflow-markdown-preview',
        'vditor-reset',
        className
      )}
      {...previewProps}
    />
  );
}

function sanitizePreview(html: string): DocumentFragment {
  const template = document.createElement('template');
  template.innerHTML = html;
  template.content
    .querySelectorAll('img,video,audio,iframe,object,embed,source')
    .forEach((element) => element.remove());
  template.content.querySelectorAll('a').forEach((anchor) => {
    const href = anchor.getAttribute('href');
    if (!href || /^(?:https?:)?\/\//i.test(href))
      anchor.removeAttribute('href');
    anchor.removeAttribute('target');
  });
  return template.content;
}

function disposeEditor(editor: Vditor) {
  try {
    editor.destroy();
  } catch {
    // Vditor may still be settling its bundled Lute initialization.
  }
}

function joinClassNames(...classNames: Array<string | undefined>) {
  return classNames.filter(Boolean).join(' ');
}
