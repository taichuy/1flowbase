'use strict';
const http = require('node:http');
const { acceptWebSocket, createFrameReader, sendClose, sendJson } = require('../mock-upstream/websocket');

async function createMock(mode) {
  const trace = [];
  const sockets = new Set();
  let requestCount = 0;
  let sequence = 0;
  const server = http.createServer((req, res) => {
    trace.push({ kind: 'http', method: req.method, path: req.url });
    if (req.url.includes('/models')) {
      res.writeHead(200, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ data: [{ id: 'gpt-6-luna', object: 'model' }] }));
    } else {
      res.writeHead(503, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ error: { message: 'controlled mock: unexpected HTTP fallback' } }));
    }
  });
  server.on('upgrade', (req, socket, head) => {
    if (!acceptWebSocket(req, socket)) return;
    sockets.add(socket);
    socket.on('close', () => sockets.delete(socket));
    createFrameReader(socket, raw => {
      const request = JSON.parse(raw);
      const ordinal = ++requestCount;
      trace.push({ kind: 'request', ordinal, type: request.type, model: request.model,
        previous_response_id: request.previous_response_id,
        reasoning: request.reasoning, input_types: (Array.isArray(request.input) ? request.input : []).map(x => x.type) });
      function emit(value) {
        value.sequence_number = sequence++;
        trace.push({ kind: 'sent', ordinal, ...value });
        sendJson(socket, value);
      }
      const isSeed = mode === 'gateway_chain' && ordinal === 1;
      const failBeforeOutput = (mode === 'retry_partial' && ordinal === 1)
        || (mode === 'gateway_chain' && ordinal === 2);
      const healthy = mode === 'healthy' || isSeed;
      if (mode === 'gateway_chain' && ordinal >= 4) {
        emit({type:'response.created',response:{id:`resp_fixture_${ordinal}`,status:'in_progress'}});
        const message = {type:'message',id:`msg_recovered_${ordinal}`,role:'assistant',status:'completed',content:[{type:'output_text',text:'recovered'}]};
        emit({type:'response.output_item.added',output_index:0,item:{...message,status:'in_progress',content:[]}});
        emit({type:'response.output_text.delta',output_index:0,content_index:0,item_id:message.id,delta:'recovered'});
        emit({type:'response.output_text.done',output_index:0,content_index:0,item_id:message.id,text:'recovered'});
        emit({type:'response.output_item.done',output_index:0,item:message});
        emit({type:'response.completed',response:{id:`resp_fixture_${ordinal}`,status:'completed',output:[message],usage:{input_tokens:10,output_tokens:10,total_tokens:20}}});
        return;
      }
      const responseId = `resp_fixture_${ordinal}`;
      emit({ type: 'response.created', response: { id: responseId, status: 'in_progress' } });
      if (failBeforeOutput) {
        trace.push({ kind: 'close', ordinal, code: 1011, reason: 'upstream websocket proxy failed', after: 'created' });
        sendClose(socket, 1011, 'upstream websocket proxy failed');
        return;
      }
      const callId = isSeed ? 'call_resume_seed' : 'call_fixture_partial';
      const item = { type: 'custom_tool_call', id: `ctc_fixture_${ordinal}`, call_id: callId,
        name: 'exec', input: '' };
      emit({ type: 'response.output_item.added', response_id: responseId, output_index: 0, item });
      const fullInput = 'fixture:'.padEnd(58, 'x');
      for (const delta of fullInput) emit({ type: 'response.custom_tool_call_input.delta',
        response_id: responseId, output_index: 0, item_id: item.id, delta });
      if (healthy) {
        emit({ type: 'response.custom_tool_call_input.done', response_id: responseId,
          output_index: 0, item_id: item.id, input: fullInput });
        const completedItem = { ...item, input: fullInput, status: 'completed' };
        emit({ type: 'response.output_item.done', response_id: responseId, output_index: 0, item: completedItem });
        emit({ type: 'response.completed', response: { id: responseId, status: 'completed',
          output: [completedItem], usage: { input_tokens: 10, output_tokens: 10, total_tokens: 20 } } });
      } else {
        trace.push({ kind: 'close', ordinal, code: 1011, reason: 'upstream websocket proxy failed', after: '58_deltas_no_done' });
        sendClose(socket, 1011, 'upstream websocket proxy failed');
      }
    }, kind => trace.push({ kind: 'socket_closed', detail: kind }));
    if (head.length) socket.emit('data', head);
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  return { baseUrl: `http://127.0.0.1:${server.address().port}`, trace,
    async close() { for (const socket of sockets) socket.destroy(); await new Promise(resolve => server.close(resolve)); } };
}
module.exports = { createMock };
