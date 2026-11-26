import {
  takeDocumentLoginEntryDiscovery,
  discardDocumentLoginEntryDiscovery
} from './document-start-discovery';
import { fetchLoginEntries } from './session';

type Request = ReturnType<typeof fetchLoginEntries>;
let initial: Request | undefined;

// Navigation-scoped handoff, not a TTL cache: keep the response until the
// first mount (even if it finished while JS was downloading), then consume it.
export function startLoginEntryDiscovery() {
  initial ??= takeDocumentLoginEntryDiscovery() ?? fetchLoginEntries();
  void initial.catch(() => {}); // the consumer handles discovery failure
}

export function consumeLoginEntryDiscovery() {
  // React StrictMode's setup/cleanup/setup belongs to the same visit. Share the
  // promise through this turn, then release it: a later visit starts fresh.
  startLoginEntryDiscovery();
  const result = initial!;
  queueMicrotask(() => {
    if (initial === result) initial = undefined;
  });
  return result;
}

export function discardLoginEntryDiscovery() {
  initial = undefined;
  discardDocumentLoginEntryDiscovery();
}
