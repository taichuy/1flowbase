'use strict';

const crypto = require('node:crypto');

function encodeLength(len) {
  if (len < 0x80) return Buffer.from([len]);
  const bytes = [];
  let remaining = len;
  while (remaining > 0) {
    bytes.unshift(remaining & 0xff);
    remaining >>= 8;
  }
  return Buffer.from([0x80 | bytes.length, ...bytes]);
}

function derTlv(tag, content) {
  const body = Buffer.isBuffer(content) ? content : Buffer.from(content);
  return Buffer.concat([Buffer.from([tag]), encodeLength(body.length), body]);
}

function derSequence(items) {
  return derTlv(0x30, Buffer.concat(items));
}

function derSet(items) {
  return derTlv(0x31, Buffer.concat(items));
}

function derOid(oidString) {
  const parts = oidString.split('.').map(Number);
  const bytes = [40 * parts[0] + parts[1]];
  for (let i = 2; i < parts.length; i += 1) {
    let val = parts[i];
    if (val === 0) {
      bytes.push(0);
      continue;
    }
    const stack = [];
    while (val > 0) {
      stack.unshift(val & 0x7f);
      val >>= 7;
    }
    for (let j = 0; j < stack.length - 1; j += 1) {
      bytes.push(stack[j] | 0x80);
    }
    bytes.push(stack[stack.length - 1]);
  }
  return derTlv(0x06, Buffer.from(bytes));
}

function derInteger(buf) {
  let body = Buffer.isBuffer(buf) ? buf : Buffer.from([buf]);
  while (body.length > 1 && body[0] === 0x00 && (body[1] & 0x80) === 0) {
    body = body.subarray(1);
  }
  if (body[0] & 0x80) {
    body = Buffer.concat([Buffer.from([0x00]), body]);
  }
  return derTlv(0x02, body);
}

function derUtf8String(str) {
  return derTlv(0x0c, Buffer.from(str, 'utf8'));
}

function derUtcTime(date) {
  const pad = (n) => String(n).padStart(2, '0');
  const yy = pad(date.getUTCFullYear() % 100);
  const mm = pad(date.getUTCMonth() + 1);
  const dd = pad(date.getUTCDate());
  const hh = pad(date.getUTCHours());
  const mi = pad(date.getUTCMinutes());
  const ss = pad(date.getUTCSeconds());
  return derTlv(0x17, Buffer.from(`${yy}${mm}${dd}${hh}${mi}${ss}Z`, 'ascii'));
}

function derBitString(buf) {
  return derTlv(0x03, Buffer.concat([Buffer.from([0x00]), buf]));
}

function toPem(label, derBuffer) {
  const base64 = derBuffer.toString('base64');
  const lines = base64.match(/.{1,64}/g) || [];
  return `-----BEGIN ${label}-----\n${lines.join('\n')}\n-----END ${label}-----\n`;
}

/**
 * Generates an ephemeral self-signed X.509 v3 ECDSA P-256 certificate for 127.0.0.1 / localhost
 * using pure Node built-in `node:crypto` (zero external `openssl` CLI or npm packages).
 */
function generateLoopbackTlsCredentials() {
  const { privateKey, publicKey } = crypto.generateKeyPairSync('ec', {
    namedCurve: 'prime256v1',
  });
  const spkiDer = publicKey.export({ type: 'spki', format: 'der' });
  const pkcs8Pem = privateKey.export({ type: 'pkcs8', format: 'pem' });

  const now = new Date();
  const notBefore = new Date(now.getTime() - 60_000);
  const notAfter = new Date(now.getTime() + 7 * 24 * 3600 * 1000);

  // ecdsa-with-SHA256: 1.2.840.10045.4.3.2
  const sigAlg = derSequence([derOid('1.2.840.10045.4.3.2')]);
  // CN=127.0.0.1 (2.5.4.3)
  const name = derSequence([
    derSet([derSequence([derOid('2.5.4.3'), derUtf8String('127.0.0.1')])]),
  ]);

  // SubjectAltName (2.5.29.17): dNSName [2] "localhost", iPAddress [7] 127.0.0.1
  const sanValue = derSequence([
    derTlv(0x82, Buffer.from('localhost', 'ascii')),
    derTlv(0x87, Buffer.from([127, 0, 0, 1])),
  ]);
  const extensions = derTlv(
    0xa3,
    derSequence([
      derSequence([derOid('2.5.29.17'), derTlv(0x04, sanValue)]),
    ]),
  );

  const tbsCert = derSequence([
    derTlv(0xa0, derInteger(2)), // v3
    derInteger(crypto.randomBytes(12)),
    sigAlg,
    name,
    derSequence([derUtcTime(notBefore), derUtcTime(notAfter)]),
    name,
    spkiDer,
    extensions,
  ]);

  const signature = crypto.sign('sha256', tbsCert, privateKey);
  const certDer = derSequence([tbsCert, sigAlg, derBitString(signature)]);

  return {
    key: pkcs8Pem,
    cert: toPem('CERTIFICATE', certDer),
  };
}

module.exports = {
  generateLoopbackTlsCredentials,
};
