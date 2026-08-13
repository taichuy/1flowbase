declare module '@1flowbase/rich-text' {
  import type { ComponentType } from 'react';

  export interface RichTextApiRequest {
    readonly body?: unknown;
  }
  export interface RichTextApi {
    post<TResponse = unknown>(
      path: string,
      request?: RichTextApiRequest
    ): Promise<TResponse>;
  }
  export type VditorEditorMode = 'ir' | 'wysiwyg' | 'sv';
  export type VditorPreviewMode = 'both' | 'editor';
  export type VditorTheme = 'classic' | 'dark';
  export interface VditorEditorHandle {
    getHTML(): string;
    getValue(): string;
    insertValue(value: string): void;
    focus(): void;
    blur(): void;
    setPreviewMode(mode: VditorPreviewMode): void;
  }
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
  export const VditorEditor: ComponentType<VditorEditorProps>;
}
