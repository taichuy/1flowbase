'use strict';

const crypto = require('node:crypto');

const WEBSOCKET_GUID = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11';

function acceptWebSocket(request, socket) {
  const key = request.headers['sec-websocket-key'];
  if (request.headers.upgrade?.toLowerCase() !== 'websocket' || typeof key !== 'string') {
    socket.end('HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n');
    return false;
  }
  const accept = crypto.createHash('sha1').update(`${key}${WEBSOCKET_GUID}`).digest('base64');
  socket.write([
    'HTTP/1.1 101 Switching Protocols',
    'Upgrade: websocket',
    'Connection: Upgrade',
    `Sec-WebSocket-Accept: ${accept}`,
    '',
    '',
  ].join('\r\n'));
  return true;
}

function encodeFrame(opcode, payload = Buffer.alloc(0)) {
  const body = Buffer.isBuffer(payload) ? payload : Buffer.from(payload);
  if (body.length > 0xffff) throw new Error('mock WebSocket frame exceeds 65535 bytes');
  const headerLength = body.length < 126 ? 2 : 4;
  const frame = Buffer.allocUnsafe(headerLength + body.length);
  frame[0] = 0x80 | opcode;
  if (body.length < 126) {
    frame[1] = body.length;
  } else {
    frame[1] = 126;
    frame.writeUInt16BE(body.length, 2);
  }
  body.copy(frame, headerLength);
  return frame;
}

function sendJson(socket, value) {
  if (!socket.destroyed) socket.write(encodeFrame(0x1, JSON.stringify(value)));
}

function sendClose(socket, code = 1000, reason = '') {
  if (socket.destroyed) return;
  const reasonBytes = Buffer.from(reason);
  const payload = Buffer.allocUnsafe(2 + reasonBytes.length);
  payload.writeUInt16BE(code, 0);
  reasonBytes.copy(payload, 2);
  socket.end(encodeFrame(0x8, payload));
}

function createFrameReader(socket, onMessage, onClose) {
  let buffered = Buffer.alloc(0);
  let closed = false;

  const closeOnce = (kind) => {
    if (closed) return;
    closed = true;
    onClose(kind);
  };

  socket.on('data', (chunk) => {
    buffered = Buffer.concat([buffered, chunk]);
    while (buffered.length >= 2) {
      const first = buffered[0];
      const second = buffered[1];
      const opcode = first & 0x0f;
      const masked = (second & 0x80) !== 0;
      let length = second & 0x7f;
      let offset = 2;
      if (length === 126) {
        if (buffered.length < 4) return;
        length = buffered.readUInt16BE(2);
        offset = 4;
      } else if (length === 127) {
        socket.destroy(new Error('mock WebSocket does not accept 64-bit frames'));
        return;
      }
      const maskLength = masked ? 4 : 0;
      if (buffered.length < offset + maskLength + length) return;
      const mask = masked ? buffered.subarray(offset, offset + 4) : null;
      offset += maskLength;
      const payload = Buffer.from(buffered.subarray(offset, offset + length));
      buffered = buffered.subarray(offset + length);
      if (mask) {
        for (let index = 0; index < payload.length; index += 1) payload[index] ^= mask[index % 4];
      }
      if (opcode === 0x8) {
        sendClose(socket);
        closeOnce('peer-close');
        return;
      }
      if (opcode === 0x9) {
        socket.write(encodeFrame(0xA, payload));
      } else if (opcode === 0x1) {
        onMessage(payload.toString('utf8'));
      }
    }
  });
  socket.on('end', () => closeOnce('end'));
  socket.on('close', () => closeOnce('close'));
  socket.on('error', () => closeOnce('error'));
}

module.exports = { acceptWebSocket, createFrameReader, sendClose, sendJson };
