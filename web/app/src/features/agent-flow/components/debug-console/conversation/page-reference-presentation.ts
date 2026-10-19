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
