import { VditorEditor } from '@1flowbase/rich-text';
import '@1flowbase/rich-text/styles.css';
import 'vditor/dist/index.css';

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
    <VditorEditor
      ariaLabel={ariaLabel}
      className={className}
      height={height}
      outline={false}
      value={value}
      onChange={onChange ?? (() => undefined)}
    />
  );
}
