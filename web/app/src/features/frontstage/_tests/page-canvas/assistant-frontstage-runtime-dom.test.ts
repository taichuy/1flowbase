import { beforeEach, describe, expect, test, vi } from 'vitest';

import { createFrontstageAssistantDomRuntime } from '../../lib/assistant-frontstage-runtime-dom';

function mountBlock() {
  const block = document.createElement('div');
  block.setAttribute('data-flowbase-frontstage-block-id', 'block-1');
  block.setAttribute('data-flowbase-frontstage-render-status', 'ready');
  block.setAttribute('data-flowbase-frontstage-generation', '3');
  const host = document.createElement('div');
  const shadow = host.attachShadow({ mode: 'open' });
  const button = document.createElement('button');
  button.textContent = '提交订单';
  shadow.append(button);
  block.append(host);
  document.body.append(block);
  return { block, button };
}

describe('Frontstage Assistant DOM runtime', () => {
  beforeEach(() => document.body.replaceChildren());

  test('AC-004 inspects and searches a bounded open Shadow DOM projection', async () => {
    mountBlock();
    const runtime = createFrontstageAssistantDomRuntime({ recompile: vi.fn() });
    const inspection = await runtime.execute('inspect_block_render', {
      block_id: 'block-1'
    });
    expect(inspection).toMatchObject({
      is_error: false,
      result: {
        render_status: 'ready',
        instance_epoch: 3,
        trust: 'untrusted_page_content'
      }
    });
    expect(JSON.stringify(inspection.result)).toContain('提交订单');
    expect(
      await runtime.execute('search_block_render', {
        block_id: 'block-1',
        query: '订单'
      })
    ).toMatchObject({
      is_error: false,
      result: { matches: [{ tag: 'button', clickable: true }] }
    });
  });

  test('AC-005 only clicks current node refs and rejects stale generations', async () => {
    const { block, button } = mountBlock();
    const onClick = vi.fn();
    button.addEventListener('click', onClick);
    const runtime = createFrontstageAssistantDomRuntime({ recompile: vi.fn() });
    const search = await runtime.execute('search_block_render', {
      block_id: 'block-1',
      query: '提交'
    });
    const result = search.result as {
      render_ref: string;
      matches: Array<{ node_ref: string }>;
    };
    await runtime.execute('click_block_element', {
      block_id: 'block-1',
      render_ref: result.render_ref,
      node_ref: result.matches[0]!.node_ref
    });
    expect(onClick).toHaveBeenCalledOnce();
    block.setAttribute('data-flowbase-frontstage-generation', '4');
    expect(
      await runtime.execute('click_block_element', {
        block_id: 'block-1',
        render_ref: result.render_ref,
        node_ref: result.matches[0]!.node_ref
      })
    ).toMatchObject({
      is_error: true,
      result: { code: 'stale_render_reference' }
    });

    const replacement = mountBlock().block;
    replacement.setAttribute('data-flowbase-frontstage-generation', '3');
    block.remove();
    expect(
      await runtime.execute('click_block_element', {
        block_id: 'block-1',
        render_ref: result.render_ref,
        node_ref: result.matches[0]!.node_ref
      })
    ).toMatchObject({
      is_error: true,
      result: { code: 'stale_render_reference' }
    });
  });

  test('AC-006 recompiles only the named block and invalidates refs', async () => {
    mountBlock();
    const recompile = vi.fn();
    const runtime = createFrontstageAssistantDomRuntime({ recompile });
    const inspection = await runtime.execute('inspect_block_render', {
      block_id: 'block-1'
    });
    const renderRef = (inspection.result as { render_ref: string }).render_ref;
    await runtime.execute('recompile_block', { block_id: 'block-1' });
    expect(recompile).toHaveBeenCalledWith('block-1');
    expect(
      await runtime.execute('read_block_render_fragment', {
        block_id: 'block-1',
        render_ref: renderRef
      })
    ).toMatchObject({
      is_error: true,
      result: { code: 'stale_render_reference' }
    });
  });
});
