import { sha256Bytes } from '@1flowbase/page-runtime';

function bytesToHex(bytes: Uint8Array) {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join(
    ''
  );
}

export async function sha256ArrayBuffer(buffer: ArrayBuffer) {
  const subtleDigest = window.crypto?.subtle?.digest;
  if (subtleDigest) {
    try {
      const digest = await subtleDigest.call(
        window.crypto.subtle,
        'SHA-256',
        buffer
      );
      return `sha256:${bytesToHex(new Uint8Array(digest))}`;
    } catch {
      // Fall through to the shared synchronous implementation.
    }
  }

  return `sha256:${sha256Bytes(new Uint8Array(buffer))}`;
}
