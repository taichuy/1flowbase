'use strict';
const http = require('node:http');
const { acceptWebSocket, createFrameReader, sendClose, sendJson } = require('../mock-upstream/websocket');
const { responsesEvents } = require('../mock-upstream/protocol-events');

async function createHttpRecoveryMock() {
  const trace = [];
  const sockets = new Set();
  function events(body, ordinal) {
    const value = responsesEvents(`http_recovery_${ordinal}`, 'HTTP_', 'RECOVERED');
    value.chunks[0].response.model = body.model;
    value.terminal.response.model = body.model;
    return [...value.chunks, value.terminal];
  }
  const server = http.createServer(async (req, res) => {
    if (req.url === '/v1/models') {
      res.writeHead(200, { 'content-type': 'application/json' });
      res.end(JSON.stringify({ data: [{ id: 'gpt-6-luna', object: 'model' }] }));
      return;
    }
    if (req.method !== 'POST' || req.url !== '/v1/responses') {
      res.writeHead(404).end();
      return;
    }
    const chunks = [];
    for await (const chunk of req) chunks.push(chunk);
    const body = JSON.parse(Buffer.concat(chunks).toString());
    trace.push({ transport: 'http', body });
    res.writeHead(200, { 'content-type': 'text/event-stream' });
    for (const event of events(body, trace.length)) res.write(`data: ${JSON.stringify(event)}\n\n`);
    res.end('data: [DONE]\n\n');
  });
  server.on('connection', socket => {
    sockets.add(socket);
    socket.once('close', () => sockets.delete(socket));
  });
  server.on('upgrade', (req, socket, head) => {
    if (!acceptWebSocket(req, socket)) return;
    createFrameReader(socket, raw => {
      const body = JSON.parse(raw);
      trace.push({ transport: 'websocket', body });
      sendJson(socket, { type: 'response.created', sequence_number: 0,
        response: { id: `resp_closed_${trace.length}`, status: 'in_progress', output: [] } });
      sendClose(socket, 1008, 'fixture-close-only; token=private-canary');
    }, () => {});
    if (head.length) socket.emit('data', head);
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  return { baseUrl: `http://127.0.0.1:${server.address().port}`, trace,
    async close() {
      for (const socket of sockets) socket.destroy();
      await new Promise(resolve => server.close(resolve));
    } };
}
module.exports = { createHttpRecoveryMock };
