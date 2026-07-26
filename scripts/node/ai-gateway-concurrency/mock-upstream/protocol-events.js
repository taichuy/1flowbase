'use strict';

const path = require('node:path');

const DEFAULT_BARRIER_MARKERS = Object.freeze({
  first: 'chunk-1',
  second: 'chunk-2',
  clientFirst: 'marker-1',
  clientSecond: 'marker-2',
});

function posixShellArgument(value) {
  return `'${value.replaceAll("'", `'"'"'`)}'`;
}

function responsesEvents(nonce, firstText = `${nonce}:chunk-1`, secondText = `${nonce}:chunk-2`) {
  const itemId = `item_${nonce}`;
  const outputText = `${firstText}${secondText}`;
  const openItem = {
    id: itemId,
    type: 'message',
    role: 'assistant',
    status: 'in_progress',
    content: [],
  };
  const openPart = { type: 'output_text', text: '', annotations: [] };
  const completedPart = { ...openPart, text: outputText };
  const completedItem = {
    ...openItem,
    status: 'completed',
    content: [completedPart],
  };
  const response = {
    id: `resp_${nonce}`,
    object: 'response',
    status: 'in_progress',
    model: 'mock-model',
    output: [],
  };
  return {
    chunks: [
      { type: 'response.created', sequence_number: 0, response },
      {
        type: 'response.output_item.added', sequence_number: 1,
        output_index: 0, item: openItem,
      },
      {
        type: 'response.content_part.added', sequence_number: 2,
        item_id: itemId, output_index: 0, content_index: 0, part: openPart,
      },
      {
        type: 'response.output_text.delta',
        sequence_number: 3,
        item_id: itemId,
        output_index: 0,
        content_index: 0,
        delta: firstText,
      },
      {
        type: 'response.output_text.delta',
        sequence_number: 4,
        item_id: itemId,
        output_index: 0,
        content_index: 0,
        delta: secondText,
      },
      {
        type: 'response.output_text.done', sequence_number: 5,
        item_id: itemId, output_index: 0, content_index: 0, text: outputText,
      },
      {
        type: 'response.content_part.done', sequence_number: 6,
        item_id: itemId, output_index: 0, content_index: 0, part: completedPart,
      },
      {
        type: 'response.output_item.done', sequence_number: 7,
        output_index: 0, item: completedItem,
      },
    ],
    terminal: {
      type: 'response.completed',
      sequence_number: 8,
      response: {
        ...response,
        status: 'completed',
        output: [completedItem],
      },
    },
    cancelled: {
      type: 'response.cancelled',
      sequence_number: 8,
      response: { ...response, status: 'cancelled' },
    },
  };
}

function responsesObservableItemEvents(nonce, firstText, secondText) {
  const response = {
    id: `resp_${nonce}`,
    object: 'response',
    status: 'in_progress',
    model: 'mock-model',
    output: [],
  };
  const texts = [firstText, secondText];
  const completedItems = texts.map((text, index) => ({
    id: `item_${nonce}_${index}`,
    type: 'message',
    role: 'assistant',
    status: 'completed',
    content: [{ type: 'output_text', text, annotations: [] }],
  }));
  const chunks = [{ type: 'response.created', sequence_number: 0, response }];
  for (const [outputIndex, completedItem] of completedItems.entries()) {
    const sequenceStart = chunks.length;
    const openItem = { ...completedItem, status: 'in_progress', content: [] };
    const openPart = { type: 'output_text', text: '', annotations: [] };
    const completedPart = completedItem.content[0];
    chunks.push(
      {
        type: 'response.output_item.added', sequence_number: sequenceStart,
        output_index: outputIndex, item: openItem,
      },
      {
        type: 'response.content_part.added', sequence_number: sequenceStart + 1,
        item_id: completedItem.id, output_index: outputIndex, content_index: 0, part: openPart,
      },
      {
        type: 'response.output_text.delta', sequence_number: sequenceStart + 2,
        item_id: completedItem.id, output_index: outputIndex, content_index: 0, delta: texts[outputIndex],
      },
      {
        type: 'response.output_text.done', sequence_number: sequenceStart + 3,
        item_id: completedItem.id, output_index: outputIndex, content_index: 0, text: texts[outputIndex],
      },
      {
        type: 'response.content_part.done', sequence_number: sequenceStart + 4,
        item_id: completedItem.id, output_index: outputIndex, content_index: 0, part: completedPart,
      },
      {
        type: 'response.output_item.done', sequence_number: sequenceStart + 5,
        output_index: outputIndex, item: completedItem,
      },
    );
  }
  return {
    barrierEvent: 'response.output_item.done',
    chunks,
    terminal: {
      type: 'response.completed',
      sequence_number: chunks.length,
      response: { ...response, status: 'completed', output: completedItems },
    },
  };
}

function anthropicEvents(nonce, firstText = `${nonce}:chunk-1`, secondText = `${nonce}:chunk-2`) {
  return {
    chunks: [
      {
        event: 'message_start',
        data: {
          type: 'message_start',
          message: {
            id: `msg_${nonce}`,
            type: 'message',
            role: 'assistant',
            model: 'mock-model',
            content: [],
            stop_reason: null,
            stop_sequence: null,
            usage: { input_tokens: 1, output_tokens: 0 },
          },
        },
      },
      {
        event: 'content_block_start',
        data: {
          type: 'content_block_start',
          index: 0,
          content_block: { type: 'text', text: '' },
        },
      },
      {
        event: 'content_block_delta',
        data: {
          type: 'content_block_delta',
          index: 0,
          delta: { type: 'text_delta', text: firstText },
        },
      },
      {
        event: 'content_block_delta',
        data: {
          type: 'content_block_delta',
          index: 0,
          delta: { type: 'text_delta', text: secondText },
        },
      },
      {
        event: 'content_block_stop',
        data: { type: 'content_block_stop', index: 0 },
      },
    ],
    terminal: [
      {
        event: 'message_delta',
        data: {
          type: 'message_delta',
          delta: { stop_reason: 'end_turn', stop_sequence: null },
          usage: { output_tokens: 2 },
        },
      },
      { event: 'message_stop', data: { type: 'message_stop' } },
    ],
  };
}

function responsesToolEvents(
  nonce,
  toolPath,
  final = false,
  executorProbeUrl = null,
  toolName = 'shell_command',
) {
  if (final) return responsesObservableItemEvents(
    nonce,
    `${DEFAULT_BARRIER_MARKERS.first} ${DEFAULT_BARRIER_MARKERS.clientFirst}`,
    `${DEFAULT_BARRIER_MARKERS.second} ${DEFAULT_BARRIER_MARKERS.clientSecond} 1flowbase gateway tool sentinel ok`
  );
  const response = { id: `resp_${nonce}`, object: 'response', status: 'in_progress', model: 'mock-model', output: [] };
  const item = {
    id: `item_${nonce}`, type: 'function_call', call_id: `call_${nonce}`,
    name: toolName, status: 'completed', arguments: JSON.stringify(toolName === 'read'
      ? { filePath: toolPath }
      : {
        command: executorProbeUrl
          ? `curl -fsS -X POST ${posixShellArgument(executorProbeUrl)}`
          : `cat -- ${posixShellArgument(toolPath)}`,
        workdir: path.dirname(toolPath),
      }),
  };
  return {
    chunks: [
      { type: 'response.created', sequence_number: 0, response },
      { type: 'response.output_item.added', sequence_number: 1, output_index: 0, item },
      { type: 'response.output_item.done', sequence_number: 2, output_index: 0, item },
    ],
    terminal: { type: 'response.completed', sequence_number: 3, response: { ...response, status: 'completed', output: [item] } },
  };
}

function anthropicToolEvents(nonce, toolPath, final = false) {
  if (final) return anthropicEvents(
    nonce,
    `${DEFAULT_BARRIER_MARKERS.first} ${DEFAULT_BARRIER_MARKERS.clientFirst}`,
    `${DEFAULT_BARRIER_MARKERS.second} ${DEFAULT_BARRIER_MARKERS.clientSecond} 1flowbase gateway tool sentinel ok`
  );
  return {
    chunks: [
      {
        event: 'message_start', data: { type: 'message_start', message: {
          id: `msg_${nonce}`, type: 'message', role: 'assistant', model: 'mock-model', content: [],
          stop_reason: null, stop_sequence: null, usage: { input_tokens: 1, output_tokens: 0 },
        } },
      },
      { event: 'content_block_start', data: { type: 'content_block_start', index: 0, content_block: {
        type: 'tool_use', id: `toolu_${nonce}`, name: 'Read', input: { file_path: toolPath },
      } } },
      { event: 'content_block_stop', data: { type: 'content_block_stop', index: 0 } },
    ],
    terminal: [
      { event: 'message_delta', data: { type: 'message_delta', delta: { stop_reason: 'tool_use', stop_sequence: null }, usage: { output_tokens: 1 } } },
      { event: 'message_stop', data: { type: 'message_stop' } },
    ],
  };
}

function chatToolEvents(nonce, toolPath, final = false) {
  if (final) return {
    doneSentinel: true,
    chunks: [{ id: `chatcmpl_${nonce}`, object: 'chat.completion.chunk', choices: [{ index: 0, delta: {
      content: `${DEFAULT_BARRIER_MARKERS.first} ${DEFAULT_BARRIER_MARKERS.clientFirst}`,
    }, finish_reason: null }] }],
    terminal: { id: `chatcmpl_${nonce}`, object: 'chat.completion.chunk', choices: [{ index: 0, delta: {
      content: `${DEFAULT_BARRIER_MARKERS.second} ${DEFAULT_BARRIER_MARKERS.clientSecond} 1flowbase gateway tool sentinel ok`,
    }, finish_reason: 'stop' }] },
  };
  return {
    doneSentinel: true,
    chunks: [{ id: `chatcmpl_${nonce}`, object: 'chat.completion.chunk', choices: [{ index: 0, delta: {
      role: 'assistant', tool_calls: [{ index: 0, id: `call_${nonce}`, type: 'function', function: {
        name: 'read', arguments: JSON.stringify({ filePath: toolPath }),
      } }],
    }, finish_reason: null }] }],
    terminal: { id: `chatcmpl_${nonce}`, object: 'chat.completion.chunk', choices: [{
      index: 0, delta: {}, finish_reason: 'tool_calls',
    }] },
  };
}

function chatTextEvents(nonce) {
  return {
    doneSentinel: true,
    chunks: [{ id: `chatcmpl_${nonce}`, object: 'chat.completion.chunk', choices: [{
      index: 0, delta: { role: 'assistant', content: '1flowbase gateway sentinel ok' }, finish_reason: null,
    }] }],
    terminal: { id: `chatcmpl_${nonce}`, object: 'chat.completion.chunk', choices: [{
      index: 0, delta: {}, finish_reason: 'stop',
    }] },
  };
}

const LOSSLESS_SENTINEL_SEGMENTS = Object.freeze([
  'same  same',
  '  ',
  '\n',
  '\n',
  '```markdown\n',
  '`same`  **same**',
  '\n```\n',
  '中文边界',
  '🙂🚀',
  '',
  '  same  same  ',
]);

const LOSSLESS_LONG_TEXT = '长文本🙂 `markdown`  repeated\n'.repeat(256);

function losslessProtocolEvents(transport, nonce, segments = LOSSLESS_SENTINEL_SEGMENTS) {
  const deltas = [...segments];
  if (transport === 'responses-sse' || transport === 'responses-websocket') {
    const response = { id: `resp_${nonce}`, object: 'response', status: 'in_progress', model: 'mock-model', output: [] };
    return {
      chunks: [
        { type: 'response.created', sequence_number: 0, response },
        ...deltas.map((delta, index) => ({
          type: 'response.output_text.delta',
          sequence_number: index + 1,
          item_id: `item_${nonce}`,
          output_index: 0,
          content_index: 0,
          delta,
        })),
      ],
      terminal: {
        type: 'response.completed',
        sequence_number: deltas.length + 1,
        response: { ...response, status: 'completed' },
      },
    };
  }
  if (transport === 'chat-completions-sse') {
    return {
      doneSentinel: true,
      chunks: deltas.map((content) => ({
        id: `chatcmpl_${nonce}`,
        object: 'chat.completion.chunk',
        choices: [{ index: 0, delta: { content }, finish_reason: null }],
      })),
      terminal: {
        id: `chatcmpl_${nonce}`,
        object: 'chat.completion.chunk',
        choices: [{ index: 0, delta: {}, finish_reason: 'stop' }],
      },
    };
  }
  if (transport === 'anthropic-sse') {
    return {
      chunks: deltas.map((text) => ({
        event: 'content_block_delta',
        data: { type: 'content_block_delta', index: 0, delta: { type: 'text_delta', text } },
      })),
      terminal: [
        {
          event: 'message_delta',
          data: { type: 'message_delta', delta: { stop_reason: 'end_turn' }, usage: { output_tokens: 1 } },
        },
        { event: 'message_stop', data: { type: 'message_stop' } },
      ],
    };
  }
  throw new Error(`unsupported lossless fixture transport: ${transport}`);
}

function responsesWireEvents(nonce, vector) {
  const response = { id: `resp_${nonce}`, object: 'response', status: 'in_progress', model: 'mock-model', output: [] };
  const output = vector === 'mcp-list-call-approval'
    ? [
      {
        id: `mcp_list_${nonce}`, type: 'mcp_list_tools', server_label: 'fixture_mcp', status: 'completed',
        tools: [{ name: 'lookup', description: 'Look up a fixture value', input_schema: { type: 'object' } }],
      },
      {
        id: `mcp_call_${nonce}`, type: 'mcp_call', server_label: 'fixture_mcp', status: 'completed',
        name: 'lookup', arguments: JSON.stringify({ query: 'fixture' }), output: 'fixture result',
      },
      {
        id: `mcp_approval_${nonce}`, type: 'mcp_approval_request', server_label: 'fixture_mcp', status: 'in_progress',
        name: 'lookup', arguments: JSON.stringify({ query: 'approval fixture' }),
      },
    ]
    : ({
      'tool-search-additional-tools': ['tool_search_call'],
      'tool-search-output-additional-tools': ['tool_search_output', 'additional_tools'],
      'hosted-tools': ['file_search_call', 'program', 'shell_call'],
      'mcp-approval-continuation': [],
    }[vector] ?? ['future_gateway_drift']).map((type, index) => ({
      id: `wire_${index}_${nonce}`, type, status: 'completed',
      x_synthetic_unknown: type === 'future_gateway_drift' ? { preserve: true } : undefined,
    }));
  const chunks = [{ type: 'response.created', sequence_number: 0, response }];
  for (const [index, item] of output.entries()) {
    chunks.push({ type: 'response.output_item.added', sequence_number: chunks.length, output_index: index, item });
    chunks.push({ type: 'response.output_item.done', sequence_number: chunks.length, output_index: index, item });
  }
  return {
    chunks,
    providerOutputTypes: output.map((item) => item.type),
    terminal: {
      type: 'response.completed', sequence_number: chunks.length,
      response: { ...response, status: 'completed', output },
    },
  };
}

module.exports = {
  DEFAULT_BARRIER_MARKERS,
  LOSSLESS_LONG_TEXT,
  LOSSLESS_SENTINEL_SEGMENTS,
  anthropicEvents,
  anthropicToolEvents,
  chatTextEvents,
  chatToolEvents,
  losslessProtocolEvents,
  responsesEvents,
  responsesToolEvents,
  responsesWireEvents,
};
