import CloseOutlined from '@ant-design/icons/es/icons/CloseOutlined';
import CodeOutlined from '@ant-design/icons/es/icons/CodeOutlined';
import { Button, Tag, Tooltip, Typography } from 'antd';

import type { AgentFlowPageReference } from '../../../api/runtime';

export function pageReferenceElementLabel(reference: AgentFlowPageReference) {
  const document = new DOMParser().parseFromString(
    reference.outer_html,
    'text/html'
  );
  const element = document.body.firstElementChild;
  if (!element) return 'div';
  const id = element.id ? `#${element.id}` : '';
  const classNames = [...element.classList]
    .slice(0, 2)
    .map((className) => `.${className}`)
    .join('');
  return `${element.tagName.toLowerCase()}${id}${classNames}`;
}

export function pageReferenceByteLength(outerHtml: string) {
  return new TextEncoder().encode(outerHtml).byteLength;
}

export function PageReferenceTag({
  reference,
  onRemove,
  removeLabel
}: {
  reference: AgentFlowPageReference;
  onRemove?: () => void;
  removeLabel?: string;
}) {
  const label = pageReferenceElementLabel(reference);
  return (
    <Tooltip
      title={`${reference.page_title || reference.page_url} · ${pageReferenceByteLength(reference.outer_html)} B`}
    >
      <Tag className="agent-flow-page-reference-tag" icon={<CodeOutlined />}>
        <span>{label}</span>
        {onRemove ? (
          <Button
            aria-label={removeLabel}
            icon={<CloseOutlined />}
            size="small"
            type="text"
            onClick={onRemove}
          />
        ) : null}
      </Tag>
    </Tooltip>
  );
}

export function PageReferenceDraftRow({
  reference,
  removeLabel,
  onRemove
}: {
  reference: AgentFlowPageReference;
  removeLabel: string;
  onRemove: () => void;
}) {
  return (
    <div
      className="agent-flow-page-reference-draft"
      data-testid="assistant-page-reference-draft"
    >
      <CodeOutlined />
      <Typography.Text className="agent-flow-page-reference-draft__element">
        {pageReferenceElementLabel(reference)}
      </Typography.Text>
      <Typography.Text
        className="agent-flow-page-reference-draft__source"
        ellipsis
        type="secondary"
      >
        {reference.page_title || reference.page_url} ·{' '}
        {pageReferenceByteLength(reference.outer_html)} B
      </Typography.Text>
      <Button
        aria-label={removeLabel}
        icon={<CloseOutlined />}
        size="small"
        type="text"
        onClick={onRemove}
      />
    </div>
  );
}
