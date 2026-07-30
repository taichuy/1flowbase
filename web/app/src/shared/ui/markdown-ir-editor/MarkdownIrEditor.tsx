import { MarkdownEditor } from '@1flowbase/rich-text';

type MarkdownIrEditorProps = {
  ariaLabel?: string;
  className?: string;
  height?: number | string;
  value?: string;
  onChange?: (value: string) => void;
};

export function MarkdownIrEditor({
  ariaLabel,
  className,
  height,
  value = '',
  onChange
}: MarkdownIrEditorProps) {
  return (
    <MarkdownEditor
      ariaLabel={ariaLabel}
      className={className}
      height={height}
      value={value}
      onChange={onChange ?? (() => undefined)}
    />
  );
}
