import type {
  FrontstageAssistantExecution,
  FrontstageAssistantRuntime
} from './assistant-frontstage-runtime';

const BLOCK_ATTRIBUTE = 'data-flowbase-frontstage-block-id';
const STATUS_ATTRIBUTE = 'data-flowbase-frontstage-render-status';
const GENERATION_ATTRIBUTE = 'data-flowbase-frontstage-generation';
const MAX_FRAGMENT_CHARS = 12_000;
const DEFAULT_PAGE_CHARS = 3_000;
const CLICKABLE_SELECTOR =
  'button,a[href],input:not([type="hidden"]),select,textarea,[role="button"],[role="link"]';

interface RenderReference {
  blockId: string;
  generation: string;
  root: HTMLElement;
  html: string;
  nodes: Map<string, Element>;
}

function failed(
  code: string,
  detail?: Record<string, unknown>
): FrontstageAssistantExecution {
  return { is_error: true, result: { status: 'failed', code, ...detail } };
}

function blockElement(blockId: unknown): HTMLElement | null {
  if (typeof blockId !== 'string' || !blockId.trim()) return null;
  return (
    [...document.querySelectorAll<HTMLElement>(`[${BLOCK_ATTRIBUTE}]`)].find(
      (element) => element.getAttribute(BLOCK_ATTRIBUTE) === blockId
    ) ?? null
  );
}

function composedChildren(element: Element): Element[] {
  const children = [...element.children];
  if (element.shadowRoot) children.push(...element.shadowRoot.children);
  return children;
}

function escapeHtml(value: string): string {
  const escapes: Record<string, string> = {
    '&': '&amp;',
    '<': '&lt;',
    '>': '&gt;',
    '"': '&quot;',
    "'": '&#39;'
  };
  return value.replace(/[&<>"']/g, (character) => escapes[character]!);
}

function projectedHtml(root: Element): string {
  const visit = (element: Element): string => {
    const tag = element.tagName.toLowerCase();
    if (tag === 'script' || tag === 'style' || tag === 'template') return '';
    const attributes = [...element.attributes]
      .filter(
        ({ name }) =>
          !name.startsWith('on') &&
          !name.startsWith('data-react') &&
          name !== 'value' &&
          name !== 'srcdoc'
      )
      .map(({ name, value }) => ` ${name}="${escapeHtml(value)}"`)
      .join('');
    const text = [...element.childNodes]
      .filter((node) => node.nodeType === Node.TEXT_NODE)
      .map((node) => escapeHtml(node.textContent ?? ''))
      .join('');
    return `<${tag}${attributes}>${text}${composedChildren(element)
      .map(visit)
      .join('')}</${tag}>`;
  };
  return visit(root).slice(0, MAX_FRAGMENT_CHARS);
}

function allElements(root: Element): Element[] {
  const result: Element[] = [];
  const queue = composedChildren(root);
  while (queue.length) {
    const current = queue.shift()!;
    result.push(current);
    queue.push(...composedChildren(current));
  }
  return result;
}

export function createFrontstageAssistantDomRuntime(input: {
  recompile(blockId: string): void;
}): FrontstageAssistantRuntime {
  const references = new Map<string, RenderReference>();

  const resolveReference = (arguments_: Record<string, unknown>) => {
    const renderRef = arguments_.render_ref;
    if (typeof renderRef !== 'string') {
      return { error: failed('invalid_render_reference') };
    }
    const reference = references.get(renderRef);
    if (!reference) return { error: failed('stale_render_reference') };
    const block = blockElement(reference.blockId);
    if (
      !block ||
      block !== reference.root ||
      block.getAttribute(GENERATION_ATTRIBUTE) !== reference.generation
    ) {
      references.delete(renderRef);
      return { error: failed('stale_render_reference') };
    }
    return { reference, block };
  };

  return {
    async execute(capability, arguments_) {
      if (capability === 'list_page_blocks') {
        const blocks = [
          ...document.querySelectorAll<HTMLElement>(`[${BLOCK_ATTRIBUTE}]`)
        ].map((block) => ({
          block_id: block.getAttribute(BLOCK_ATTRIBUTE),
          render_status: block.getAttribute(STATUS_ATTRIBUTE) ?? 'unknown',
          generation: Number(block.getAttribute(GENERATION_ATTRIBUTE) ?? 0)
        }));
        return { is_error: false, result: { status: 'ok', blocks } };
      }

      if (
        capability === 'read_block_render_fragment' ||
        capability === 'click_block_element'
      ) {
        const resolved = resolveReference(arguments_);
        if (resolved.error) return resolved.error;
        if (capability === 'read_block_render_fragment') {
          const cursor = Math.max(0, Number(arguments_.cursor ?? 0));
          const limit = Math.min(
            DEFAULT_PAGE_CHARS,
            Math.max(1, Number(arguments_.limit ?? DEFAULT_PAGE_CHARS))
          );
          const fragment = resolved.reference!.html.slice(
            cursor,
            cursor + limit
          );
          const next = cursor + fragment.length;
          return {
            is_error: false,
            result: {
              status: 'ok',
              fragment,
              next_cursor: next < resolved.reference!.html.length ? next : null,
              trust: 'untrusted_page_content'
            }
          };
        }
        const nodeRef = arguments_.node_ref;
        const element =
          typeof nodeRef === 'string'
            ? resolved.reference!.nodes.get(nodeRef)
            : null;
        if (element && !element.isConnected) {
          return failed('stale_render_reference');
        }
        if (
          !(element instanceof HTMLElement) ||
          !element.matches(CLICKABLE_SELECTOR)
        ) {
          return failed('element_not_clickable');
        }
        if (
          'disabled' in element &&
          Boolean((element as HTMLButtonElement).disabled)
        ) {
          return failed('element_disabled');
        }
        element.click();
        return {
          is_error: false,
          result: {
            status: 'clicked',
            block_id: resolved.reference!.blockId,
            node_ref: nodeRef
          }
        };
      }

      const block = blockElement(arguments_.block_id);
      if (!block) return failed('block_not_mounted');
      const blockId = block.getAttribute(BLOCK_ATTRIBUTE)!;
      const generation = block.getAttribute(GENERATION_ATTRIBUTE) ?? '0';

      if (capability === 'inspect_block_render') {
        const renderRef = crypto.randomUUID();
        const html = projectedHtml(block);
        references.set(renderRef, {
          blockId,
          generation,
          root: block,
          html,
          nodes: new Map()
        });
        return {
          is_error: false,
          result: {
            status: 'ok',
            block_id: blockId,
            render_status: block.getAttribute(STATUS_ATTRIBUTE) ?? 'unknown',
            error: block.getAttribute('data-flowbase-frontstage-render-error'),
            render_ref: renderRef,
            instance_epoch: Number(generation),
            preview: html.slice(0, DEFAULT_PAGE_CHARS),
            content_truncated: html.length > DEFAULT_PAGE_CHARS,
            trust: 'untrusted_page_content'
          }
        };
      }

      if (capability === 'search_block_render') {
        const query =
          typeof arguments_.query === 'string'
            ? arguments_.query.trim().toLowerCase()
            : '';
        if (!query) return failed('invalid_query');
        const renderRef = crypto.randomUUID();
        const html = projectedHtml(block);
        const nodes = new Map<string, Element>();
        const matches = allElements(block)
          .filter((element) =>
            `${element.tagName} ${element.textContent ?? ''} ${[
              ...element.attributes
            ]
              .map(({ name, value }) => `${name} ${value}`)
              .join(' ')}`
              .toLowerCase()
              .includes(query)
          )
          .slice(0, 50)
          .map((element, index) => {
            const nodeRef = `${renderRef}:${index}`;
            nodes.set(nodeRef, element);
            return {
              node_ref: nodeRef,
              tag: element.tagName.toLowerCase(),
              text: (element.textContent ?? '').trim().slice(0, 300),
              clickable: element.matches(CLICKABLE_SELECTOR)
            };
          });
        references.set(renderRef, {
          blockId,
          generation,
          root: block,
          html,
          nodes
        });
        return {
          is_error: false,
          result: {
            status: 'ok',
            render_ref: renderRef,
            instance_epoch: Number(generation),
            matches,
            trust: 'untrusted_page_content'
          }
        };
      }

      if (capability === 'recompile_block') {
        input.recompile(blockId);
        references.clear();
        return {
          is_error: false,
          result: {
            status: 'recompile_requested',
            block_id: blockId,
            previous_generation: Number(generation)
          }
        };
      }

      return failed('unsupported_capability');
    }
  };
}
