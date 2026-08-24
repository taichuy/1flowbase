import { useEffect, useRef } from 'react';

import Vditor from 'vditor';

import { acquireBundledVditorRuntime } from './runtime-assets';

const NO_REMOTE_ASSET_BASE = '/__1flowbase_bundled_vditor__';

export interface RichTextApiRequest {
  readonly body?: unknown;
}

export interface RichTextApi {
  post<TResponse = unknown>(
    path: string,
    request?: RichTextApiRequest
  ): Promise<TResponse>;
}

export interface VditorEditorHandle {
  getHTML(): string;
  getValue(): string;
  insertValue(value: string): void;
  focus(): void;
  blur(): void;
  setPreviewMode(mode: VditorPreviewMode): void;
}

export type VditorEditorMode = 'ir' | 'wysiwyg' | 'sv';
export type VditorPreviewMode = 'both' | 'editor';
export type VditorTheme = 'classic' | 'dark';

export interface VditorEditorProps {
  readonly api?: RichTextApi;
  readonly ariaLabel?: string;
  readonly className?: string;
  readonly height?: number | string;
  readonly mode?: VditorEditorMode;
  readonly onChange: (value: string) => void;
  readonly onReady?: (editor: VditorEditorHandle | null) => void;
  readonly outline?: boolean;
  readonly placeholder?: string;
  readonly previewMode?: VditorPreviewMode;
  readonly theme?: VditorTheme;
  readonly value: string;
}

export function VditorEditor({
  api,
  ariaLabel = 'vditor_editor',
  className,
  height = 360,
  mode = 'ir',
  onChange,
  onReady,
  outline = true,
  placeholder = '',
  previewMode = 'both',
  theme = 'classic',
  value
}: VditorEditorProps) {
  const mountRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<Vditor | null>(null);
  const readyRef = useRef(false);
  const valueRef = useRef(value);
  const apiRef = useRef(api);
  const onChangeRef = useRef(onChange);
  const onReadyRef = useRef(onReady);
  const uploadEnabled = api !== undefined;

  useEffect(() => {
    apiRef.current = api;
  }, [api]);

  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  useEffect(() => {
    onReadyRef.current = onReady;
  }, [onReady]);

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
    let releaseRuntime: () => void = () => undefined;
    queueMicrotask(() => {
      if (disposed) return;
      releaseRuntime = acquireBundledVditorRuntime(
        mount.getRootNode() as Document | ShadowRoot
      );
      const toolbar: IOptions['toolbar'] = [
        'headings',
        'bold',
        'italic',
        'strike',
        'link',
        '|',
        'list',
        'ordered-list',
        'check',
        'outdent',
        'indent',
        '|',
        'quote',
        'line',
        'code',
        'inline-code',
        'insert-before',
        'insert-after',
        '|',
        ...(uploadEnabled ? (['upload'] as const) : []),
        'table',
        '|',
        'undo',
        'redo',
        '|',
        'fullscreen',
        'edit-mode',
        {
          name: 'more',
          toolbar: [
            'both',
            'code-theme',
            'content-theme',
            'outline',
            'preview',
            'info'
          ]
        }
      ];
      const editor = new Vditor(mount, {
        _lutePath: NO_REMOTE_ASSET_BASE,
        cache: { enable: false },
        cdn: NO_REMOTE_ASSET_BASE,
        height,
        hint: { emoji: {}, emojiPath: NO_REMOTE_ASSET_BASE },
        i18n: window.VditorI18n,
        image: { isPreview: true },
        mode,
        outline: { enable: outline, position: 'left' },
        placeholder,
        preview: {
          hljs: { enable: false },
          markdown: {
            codeBlockPreview: false,
            mathBlockPreview: false,
            sanitize: true
          },
          mode: previewMode,
          render: { media: { enable: true } },
          theme: { current: 'ant-design', path: NO_REMOTE_ASSET_BASE }
        },
        theme,
        toolbar,
        upload: {
          accept: 'image/*,.pdf,.txt,.md,.csv,.json,.zip',
          handler: uploadFiles,
          linkToImgUrl: '',
          multiple: true,
          url: ''
        },
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
          onReadyRef.current?.(createEditorHandle(editor));
        },
        input: (nextValue) => {
          valueRef.current = nextValue;
          onChangeRef.current(nextValue);
        }
      });
      editorRef.current = editor;

      async function uploadFiles(files: File[]): Promise<null> {
        const currentApi = apiRef.current;
        if (!currentApi) throw new Error('File upload is unavailable.');
        const links = await Promise.all(
          files.map((file) => uploadFile(currentApi, file))
        );
        editor.insertValue(`${links.join('\n')}\n`);
        return null;
      }
    });

    return () => {
      disposed = true;
      releaseRuntime();
      readyRef.current = false;
      onReadyRef.current?.(null);
      const editor = editorRef.current;
      editorRef.current = null;
      if (editor) disposeEditor(editor);
    };
  }, [height, mode, outline, placeholder, previewMode, theme, uploadEnabled]);

  return (
    <div
      ref={mountRef}
      aria-label={ariaLabel}
      className={joinClassNames('oneflow-markdown-editor', className)}
    />
  );
}

interface UploadedFileResponse {
  readonly file_table_id: string;
  readonly record: { readonly id: string };
  readonly storage_id: string;
}

async function uploadFile(api: RichTextApi, file: File): Promise<string> {
  const uploaded = await api.post<UploadedFileResponse>(
    '/api/console/files/upload',
    {
      body: {
        file: {
          base64: encodeBase64(await file.arrayBuffer()),
          content_type: file.type || 'application/octet-stream',
          file_name: file.name
        }
      }
    }
  );
  if (!uploaded.file_table_id || !uploaded.record?.id) {
    throw new Error('File upload returned an invalid resource identity.');
  }
  const contentUrl = `/api/console/files/${encodeURIComponent(uploaded.file_table_id)}/records/${encodeURIComponent(uploaded.record.id)}/content`;
  const label = escapeMarkdownLabel(file.name);
  return file.type.startsWith('image/')
    ? `![${label}](${contentUrl})`
    : `[${label}](${contentUrl})`;
}

function encodeBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }
  return btoa(binary);
}

function escapeMarkdownLabel(value: string): string {
  return value.replaceAll('\\', '\\\\').replaceAll('[', '\\[').replaceAll(']', '\\]');
}

function createEditorHandle(editor: Vditor): VditorEditorHandle {
  return {
    getHTML: () => editor.getHTML(),
    getValue: () => editor.getValue(),
    insertValue: (value) => editor.insertValue(value),
    focus: () => editor.focus(),
    blur: () => editor.blur(),
    setPreviewMode: (mode) => editor.setPreviewMode(mode)
  };
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
