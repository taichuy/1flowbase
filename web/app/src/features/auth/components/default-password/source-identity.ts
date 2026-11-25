import { sha256Text, sourceSha256 } from 'virtual:1flowbase-default-password';

/** No trimming/normalization: a single changed byte belongs to the legacy host. */
export function matchesDefaultPasswordSource(source: string): boolean {
  return sha256Text(source) === sourceSha256;
}
