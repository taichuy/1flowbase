import type { Mock } from 'vitest';

interface RichTextFixtureInstance {
  options: {
    mode?: string;
    value?: string;
    outline?: { enable?: boolean; position?: string };
    after?: () => void;
    input?: (value: string) => void;
  };
  setValue: Mock<(value: string, emitUpdate?: boolean) => void>;
  getValue: Mock<() => string>;
  destroy: Mock<() => void>;
}

export async function createRichTextEditorFixture(vditorMock: {
  constructor: unknown;
  instances: RichTextFixtureInstance[];
}) {
  const React = await import('react');

  type EditorProps = {
    ariaLabel?: string;
    className?: string;
    onChange: (value: string) => void;
    outline?: boolean;
    value: string;
  };
  type EditorInstance = RichTextFixtureInstance;
  type EditorOptions = EditorInstance['options'];
  const createEditor = vditorMock.constructor as unknown as (
    target: HTMLElement,
    options: EditorOptions
  ) => EditorInstance;

  return {
    VditorEditor({
      ariaLabel = 'vditor_editor',
      className,
      onChange,
      outline,
      value
    }: EditorProps) {
      const mountRef = React.useRef<HTMLDivElement>(null);
      const editorRef = React.useRef<EditorInstance | null>(null);
      const readyRef = React.useRef(false);
      const valueRef = React.useRef(value);
      const onChangeRef = React.useRef(onChange);

      React.useEffect(() => {
        onChangeRef.current = onChange;
      }, [onChange]);

      React.useEffect(() => {
        valueRef.current = value;
        const editor = editorRef.current;
        if (editor && readyRef.current && editor.getValue() !== value) {
          editor.setValue(value);
        }
      }, [value]);

      React.useEffect(() => {
        const mount = mountRef.current;
        if (!mount) return undefined;

        let disposed = false;
        queueMicrotask(() => {
          if (disposed) return;
          const editor = createEditor(mount, {
            mode: 'ir',
            outline: { enable: outline, position: 'left' },
            value: valueRef.current,
            after: () => {
              if (disposed) return;
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
          editor?.destroy();
        };
      }, [outline]);

      return React.createElement('div', {
        ref: mountRef,
        'aria-label': ariaLabel,
        className: ['oneflow-markdown-editor', className]
          .filter(Boolean)
          .join(' ')
      });
    }
  };
}
