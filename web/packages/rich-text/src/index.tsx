import { useEffect, useRef, useState } from 'react';

import type Vditor from 'vditor';
import { VditorFrame } from './frame';
export { VditorPreview } from './preview';

import {
  loadFrameRuntime,
  sharedPreviewOptions,
  VDITOR_ASSET_BASE,
  getBundledEditorMessages
} from './runtime-assets';

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
  readonly locale?: 'zh_Hans' | 'en_US';
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
  locale = 'zh_Hans',
  mode = 'ir',
  onChange,
  onReady,
  outline = true,
  placeholder = '',
  previewMode = 'both',
  theme = 'classic',
  value
}: VditorEditorProps) {
  const [frameDocument, setFrameDocument] = useState<Document | null>(null);
  const [failed, setFailed] = useState(false);
  const [loading, setLoading] = useState(true);
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
    if (!frameDocument) return undefined;
    const mount = frameDocument.createElement('div');
    frameDocument.body.appendChild(mount);

    const frame = frameDocument.defaultView?.frameElement;
    const outerDocument = frame?.ownerDocument;
    const exitFullscreen = () => {
      if (
        !outerDocument?.fullscreenElement &&
        mount.classList.contains('vditor--fullscreen')
      ) {
        mount.querySelector<HTMLElement>('[data-type="fullscreen"]')?.click();
      }
    };
    outerDocument?.addEventListener('fullscreenchange', exitFullscreen);
    let disposed = false;
    setFailed(false);
    setLoading(true);
    void loadFrameRuntime(frameDocument)
      .then((FrameVditor) => {
        if (disposed) return;
        const toolbar: IOptions['toolbar'] = [
          // Vditor 3.11.2 also gates language-hint keyboard selection on this entry.
          'emoji',
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
          {
            name: 'fullscreen',
            click: () => {
              const frame = frameDocument.defaultView
                ?.frameElement as HTMLIFrameElement | null;
              if (!frame) return;
              if (mount.classList.contains('vditor--fullscreen')) {
                void frame.requestFullscreen?.().catch(() => undefined);
              } else if (frame.matches(':fullscreen')) {
                void frame.ownerDocument.exitFullscreen();
              }
            }
          },
          'edit-mode',
          {
            name: 'more',
            toolbar: ['both', 'outline', 'preview', 'info']
          }
        ];
        const editor = new FrameVditor(mount, {
          _lutePath: `${VDITOR_ASSET_BASE}/dist/js/lute/lute.min.js`,
          cache: { enable: false },
          cdn: VDITOR_ASSET_BASE,
          height,
          hint: { emojiPath: `${VDITOR_ASSET_BASE}/dist/images/emoji` },
          i18n: getBundledEditorMessages(locale),
          lang: locale === 'en_US' ? 'en_US' : 'zh_CN',
          image: { isPreview: true },
          mode,
          outline: { enable: outline, position: 'left' },
          placeholder,
          preview: {
            ...sharedPreviewOptions(locale, theme === 'dark', frameDocument),
            mode: previewMode
          },
          theme,
          customWysiwygToolbar: () => undefined,
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
            setLoading(false);
            if (editor.getValue() !== valueRef.current) {
              editor.setValue(valueRef.current, true);
            }
            onReadyRef.current?.(createEditorHandle(editor));
          },
          input: (nextValue) => {
            if (disposed) return;
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
      })
      .catch(() => {
        if (!disposed) {
          setFailed(true);
          setLoading(false);
        }
      });

    return () => {
      const editor = editorRef.current;
      // Vditor debounces input. Capture the visible draft before rebuilding
      // so a locale change retains even input not yet sent to the consumer.
      if (editor && readyRef.current) {
        const draft = editor.getValue();
        if (draft !== valueRef.current) {
          valueRef.current = draft;
          onChangeRef.current(draft);
        }
      }
      disposed = true;

      readyRef.current = false;
      onReadyRef.current?.(null);
      editorRef.current = null;
      if (editor) disposeEditor(editor);
      outerDocument?.removeEventListener('fullscreenchange', exitFullscreen);
      mount.remove();
    };
  }, [
    frameDocument,
    height,
    locale,
    mode,
    outline,
    placeholder,
    previewMode,
    theme,
    uploadEnabled
  ]);

  return (
    <div className={joinClassNames('oneflow-markdown-editor', className)}>
      {failed && (
        <div role="alert">
          {locale === 'zh_Hans'
            ? '编辑器加载失败，请刷新重试。'
            : 'Could not load the editor. Please refresh to retry.'}
        </div>
      )}
      <VditorFrame
        loading={loading}
        loadingLabel={
          locale === 'zh_Hans' ? '编辑器加载中…' : 'Loading editor…'
        }
        title={ariaLabel}
        height={height}
        onDocument={setFrameDocument}
      />
    </div>
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
  return value
    .replaceAll('\\', '\\\\')
    .replaceAll('[', '\\[')
    .replaceAll(']', '\\]');
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
