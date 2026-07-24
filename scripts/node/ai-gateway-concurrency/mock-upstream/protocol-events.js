'use strict';

function responsesEvents(nonce, firstText = `${nonce}:chunk-1`, secondText = `${nonce}:chunk-2`) {
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
        type: 'response.output_text.delta',
        sequence_number: 1,
        item_id: `item_${nonce}`,
        output_index: 0,
        content_index: 0,
        delta: firstText,
      },
      {
        type: 'response.output_text.delta',
        sequence_number: 2,
        item_id: `item_${nonce}`,
        output_index: 0,
        content_index: 0,
        delta: secondText,
      },
    ],
    terminal: {
      type: 'response.completed',
      sequence_number: 3,
      response: {
        ...response,
        status: 'completed',
        output: [{
          id: `item_${nonce}`,
          type: 'message',
          role: 'assistant',
          status: 'completed',
          content: [{ type: 'output_text', text: `${firstText}${secondText}`, annotations: [] }],
        }],
      },
    },
    cancelled: {
      type: 'response.cancelled',
      sequence_number: 3,
      response: { ...response, status: 'cancelled' },
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

function responsesToolEvents(nonce, toolPath, final = false, executorProbeUrl = null) {
  if (final) return responsesEvents(nonce, 'marker-1', 'marker-2 1flowbase gateway tool sentinel ok');
  const response = { id: `resp_${nonce}`, object: 'response', status: 'in_progress', model: 'mock-model', output: [] };
  const item = {
    id: `item_${nonce}`, type: 'local_shell_call', call_id: `call_${nonce}`,
    status: 'completed', action: {
      type: 'exec',
      command: executorProbeUrl
        ? ['curl', '-fsS', '-X', 'POST', executorProbeUrl]
        : ['cat', toolPath],
    },
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
  if (final) return anthropicEvents(nonce, 'marker-1', 'marker-2 1flowbase gateway tool sentinel ok');
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
      content: 'marker-1',
    }, finish_reason: null }] }],
    terminal: { id: `chatcmpl_${nonce}`, object: 'chat.completion.chunk', choices: [{ index: 0, delta: {
      content: 'marker-2 1flowbase gateway tool sentinel ok',
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

function responsesWireEvents(nonce, vector) {
  const response = { id: `resp_${nonce}`, object: 'response', status: 'in_progress', model: 'mock-model', output: [] };
  const types = {
    'tool-search-additional-tools': ['tool_search_call'],
    'tool-search-output-additional-tools': ['tool_search_output', 'additional_tools'],
    'hosted-tools': ['file_search_call', 'program', 'shell_call'],
    'mcp-list-call-approval': ['mcp_list_tools', 'mcp_call', 'mcp_approval_request', 'mcp_approval_response'],
  }[vector] ?? ['future_gateway_drift'];
  const output = types.map((type, index) => ({
    id: `wire_${index}_${nonce}`, type, status: 'completed',
    x_synthetic_unknown: type === 'future_gateway_drift' ? { preserve: true } : undefined,
  }));
  const chunks = [{ type: 'response.created', sequence_number: 0, response }];
  for (const [index, item] of output.entries()) {
    chunks.push({ type: 'response.output_item.added', sequence_number: chunks.length, output_index: index, item });
    chunks.push({ type: 'response.output_item.done', sequence_number: chunks.length, output_index: index, item });
  }
  chunks.push({ type: 'response.future_gateway_drift', sequence_number: chunks.length, preserve: { opaque: true } });
  return {
    chunks,
    terminal: {
      type: 'response.completed', sequence_number: chunks.length,
      response: { ...response, status: 'completed', output },
    },
  };
}

module.exports = {
  anthropicEvents,
  anthropicToolEvents,
  chatTextEvents,
  chatToolEvents,
  responsesEvents,
  responsesToolEvents,
  responsesWireEvents,
};
