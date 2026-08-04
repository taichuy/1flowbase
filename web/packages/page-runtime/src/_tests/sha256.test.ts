import { describe, expect, test } from 'vitest';

import { sha256Bytes, sha256Text } from '../sha256';

describe('shared SHA-256 primitive', () => {
  test.each([
    ['', 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'],
    ['abc', 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'],
    [
      'The quick brown fox jumps over the lazy dog',
      'd7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592'
    ],
    [
      '你好，1flowbase',
      'da01cfa462865dad3c7e23c1a4040a8f38a54992eb89cd700db990932e0da50e'
    ]
  ])('matches the standard text vector for %j', (value, expected) => {
    expect(sha256Text(value)).toBe(expected);
  });

  test('hashes binary bytes without text conversion', () => {
    expect(sha256Bytes(new Uint8Array([0, 1, 2, 0, 255]))).toBe(
      'ef7e301027f931dfba06c7ded4ef305797f43cc115a664f9af9b57d08c3172c2'
    );
  });
});
