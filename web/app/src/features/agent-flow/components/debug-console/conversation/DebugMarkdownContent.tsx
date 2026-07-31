import { XMarkdown } from '@ant-design/x-markdown';

const MARKDOWN_CONFIG = {
  breaks: true,
  gfm: true
};
const STREAMING_MARKDOWN = { hasNextChunk: true };
const STATIC_MARKDOWN = { hasNextChunk: false };

export function DebugMarkdownContent({
  content,
  className,
  streaming = false
}: {
  content: string;
  className?: string;
  streaming?: boolean;
}) {
  return (
    <XMarkdown
      className={className}
      config={MARKDOWN_CONFIG}
      content={content}
      disableDefaultStyles
      escapeRawHtml
      streaming={streaming ? STREAMING_MARKDOWN : STATIC_MARKDOWN}
    />
  );
}
