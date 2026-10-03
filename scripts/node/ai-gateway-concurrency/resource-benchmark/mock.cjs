'use strict';
const http = require('node:http');
const { once } = require('node:events');
const { responsesEvents } = require('../mock-upstream/protocol-events');
const { DELTAS, TEXT } = require('./payload.cjs');
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
function wireEvents() {
  const stream = responsesEvents('resource-fixed', '', '');
  const template = stream.chunks.find(e => e.type === 'response.output_text.delta');
  const first = stream.chunks.findIndex(e => e.type === 'response.output_text.delta');
  const last = stream.chunks.findLastIndex(e => e.type === 'response.output_text.delta');
  const whole = TEXT.repeat(DELTAS);
  const events = [...stream.chunks.slice(0, first), ...Array.from({ length: DELTAS }, () => ({ ...template, delta: TEXT })), ...stream.chunks.slice(last + 1), stream.terminal];
  events.forEach((event, i) => {
    event.sequence_number = i;
    if (event.text !== undefined) event.text = whole;
    if (event.type === 'response.content_part.done') event.part.text = whole;
    if (event.item?.content?.[0]) event.item.content[0].text = whole;
    if (event.response?.output?.[0]?.content?.[0]) event.response.output[0].content[0].text = whole;
  });
  return events;
}
async function startMock() {
  const counters = { requests: 0, request_bytes: 0, response_bytes: 0, failures: 0, active: 0 };
  const server = http.createServer(async (request, response) => {
    try {
      if (request.method !== 'POST' || !['/v1/responses', '/responses'].includes(request.url)) { response.writeHead(404).end(); return; }
      counters.active++;
      let bytes = 0;
      const chunks = [];
      for await (const chunk of request) {
        bytes += chunk.length;
        if (bytes > 1024 * 1024) throw Error('mock body limit');
        chunks.push(chunk);
      }
      JSON.parse(Buffer.concat(chunks).toString('utf8'));
      counters.requests++;
      counters.request_bytes += bytes;
      response.writeHead(200, { 'content-type': 'text/event-stream', 'cache-control': 'no-cache' });
      response.flushHeaders();
      for (const event of wireEvents()) {
        if (response.destroyed) throw Error('mock connection closed');
        const wire = `event: ${event.type}\ndata: ${JSON.stringify(event)}\n\n`;
        counters.response_bytes += Buffer.byteLength(wire);
        if (!response.write(wire)) await once(response, 'drain', { signal: AbortSignal.timeout(5000) });
        if (event.type === 'response.output_text.delta') await delay(10);
      }
      response.end();
    } catch { counters.failures++; response.destroy(); }
    finally { if (request.method === 'POST' && ['/v1/responses', '/responses'].includes(request.url)) counters.active--; }
  });
  server.requestTimeout = 30000;
  server.headersTimeout = 10000;
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  return { baseUrl: `http://127.0.0.1:${server.address().port}`, snapshot: () => ({ ...counters }), async close() { server.closeAllConnections(); await new Promise(resolve => server.close(resolve)); } };
}
module.exports = { startMock, wireEvents, delay };
