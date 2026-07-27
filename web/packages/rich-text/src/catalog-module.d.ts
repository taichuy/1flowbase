declare module '@1flowbase/rich-text' {
  import type { ComponentType, HTMLAttributes } from 'react';

  export interface MarkdownEditorProps {
    readonly ariaLabel?: string;
    readonly className?: string;
    readonly height?: number | string;
    readonly placeholder?: string;
    readonly value: string;
    readonly onChange: (value: string) => void;
  }
  export interface MarkdownPreviewProps extends Omit<
    HTMLAttributes<HTMLDivElement>,
    'children'
  > {
    readonly value: string;
  }
  export const MarkdownEditor: ComponentType<MarkdownEditorProps>;
  export const MarkdownPreview: ComponentType<MarkdownPreviewProps>;
}
