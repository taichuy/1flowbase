'use strict';

function responsesEvents(nonce) {
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
        delta: `${nonce}:chunk-1`,
      },
      {
        type: 'response.output_text.delta',
        sequence_number: 2,
        item_id: `item_${nonce}`,
        output_index: 0,
        content_index: 0,
        delta: `${nonce}:chunk-2`,
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
          content: [{ type: 'output_text', text: `${nonce}:chunk-1${nonce}:chunk-2`, annotations: [] }],
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

function anthropicEvents(nonce) {
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
          delta: { type: 'text_delta', text: `${nonce}:chunk-1` },
        },
      },
      {
        event: 'content_block_delta',
        data: {
          type: 'content_block_delta',
          index: 0,
          delta: { type: 'text_delta', text: `${nonce}:chunk-2` },
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

module.exports = { anthropicEvents, responsesEvents };
