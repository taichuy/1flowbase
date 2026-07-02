import { useEffect, useRef } from 'react';
import Vditor from 'vditor';
import 'vditor/dist/js/i18n/zh_CN.js';
import 'vditor/dist/index.css';
import './markdown-ir-editor.css';

type MarkdownIrEditorProps = {
  ariaLabel?: string;
  className?: string;
  height?: number | string;
  value?: string;
  onChange?: (value: string) => void;
};

function deferVditorInit(callback: () => void) {
  queueMicrotask(callback);
}

function destroyVditor(editor: Vditor) {
  try {
    editor.destroy();
  } catch {
    // Vditor can still be settling its async Lute initialization during teardown.
  }
}

function joinClassNames(...classNames: Array<string | undefined>) {
  return classNames.filter(Boolean).join(' ');
}

export function MarkdownIrEditor({
  ariaLabel = 'markdown_editor',
  className,
  height = 180,
  value = '',
  onChange
}: MarkdownIrEditorProps) {
  const mountRef = useRef<HTMLDivElement | null>(null);
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
    if (!mountRef.current) {
      return undefined;
    }

    let disposed = false;

    deferVditorInit(() => {
      if (disposed || !mountRef.current) {
        return;
      }

      const editor = new Vditor(mountRef.current, {
        cache: { enable: false },
        height,
        i18n: window.VditorI18n,
        mode: 'ir',
        preview: { mode: 'editor' },
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
        value: valueRef.current,
        after: () => {
          if (disposed) {
            destroyVditor(editor);
            return;
          }

          readyRef.current = true;

          if (editor.getValue() !== valueRef.current) {
            editor.setValue(valueRef.current, true);
          }
        },
        input: (nextValue) => {
          valueRef.current = nextValue;
          onChangeRef.current?.(nextValue);
        }
      });

      editorRef.current = editor;
    });

    return () => {
      disposed = true;
      const wasReady = readyRef.current;
      readyRef.current = false;
      const editor = editorRef.current;
      editorRef.current = null;

      if (editor && wasReady) {
        destroyVditor(editor);
      }
    };
  }, [height]);

  return (
    <div
      ref={mountRef}
      aria-label={ariaLabel}
      className={joinClassNames('markdown-ir-editor', className)}
    />
  );
}
