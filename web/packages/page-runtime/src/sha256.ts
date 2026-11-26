const SHA256_INITIAL_HASH = new Uint32Array(8);
const SHA256_ROUND_CONSTANTS = new Uint32Array(64);
for (let n = 2, count = 0; count < 64; n += 1) {
  let prime = true;
  for (let d = 2; d * d <= n; d += 1) {
    if (n % d === 0) {
      prime = false;
      break;
    }
  }
  if (!prime) continue;
  if (count < 8) SHA256_INITIAL_HASH[count] = ((Math.sqrt(n) % 1) * 0x100000000) >>> 0;
  SHA256_ROUND_CONSTANTS[count] = ((Math.cbrt(n) % 1) * 0x100000000) >>> 0;
  count += 1;
}

const rotr = (v: number, b: number): number => (v >>> b) | (v << (32 - b));

export function sha256Text(value: string): string {
  return sha256Bytes(new TextEncoder().encode(value));
}

export function sha256Bytes(bytes: Uint8Array): string {
  const bitLen = bytes.length * 8;
  const padLen = Math.ceil((bytes.length + 9) / 64) * 64;
  const padded = new Uint8Array(padLen);
  padded.set(bytes);
  padded[bytes.length] = 0x80;
  const view = new DataView(padded.buffer);
  view.setUint32(padLen - 8, Math.floor(bitLen / 0x100000000));
  view.setUint32(padLen - 4, bitLen >>> 0);
  const hash = new Uint32Array(SHA256_INITIAL_HASH);
  const w = new Uint32Array(64);
  for (let off = 0; off < padLen; off += 64) {
    for (let i = 0; i < 16; i += 1) w[i] = view.getUint32(off + i * 4);
    for (let i = 16; i < 64; i += 1) {
      const l = w[i - 15]!;
      const h = w[i - 2]!;
      w[i] =
        (w[i - 16]! +
          (rotr(l, 7) ^ rotr(l, 18) ^ (l >>> 3)) +
          w[i - 7]! +
          (rotr(h, 17) ^ rotr(h, 19) ^ (h >>> 10))) >>>
        0;
    }
    let [a, b, c, d, e, f, g, h] = hash;
    for (let i = 0; i < 64; i += 1) {
      const t1 =
        (h +
          (rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)) +
          ((e & f) ^ (~e & g)) +
          SHA256_ROUND_CONSTANTS[i]! +
          w[i]!) >>>
        0;
      const t2 =
        ((rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)) +
          ((a & b) ^ (a & c) ^ (b & c))) >>>
        0;
      h = g;
      g = f;
      f = e;
      e = (d + t1) >>> 0;
      d = c;
      c = b;
      b = a;
      a = (t1 + t2) >>> 0;
    }
    const next = [a, b, c, d, e, f, g, h];
    for (let i = 0; i < 8; i += 1) hash[i] = (hash[i]! + next[i]!) >>> 0;
  }
  return Array.from(hash, (word) => word.toString(16).padStart(8, '0')).join('');
}
