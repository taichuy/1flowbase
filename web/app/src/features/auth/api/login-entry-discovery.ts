import { fetchLoginEntries } from './session';

type Request = ReturnType<typeof fetchLoginEntries>;
let initial: Request | undefined;

// Navigation-scoped handoff, not a TTL cache: keep the response until the
// first mount (even if it finished while JS was downloading), then consume it.
export function startLoginEntryDiscovery() {
  initial ??= fetchLoginEntries();
  void initial.catch(() => {}); // the consumer handles discovery failure
}

export function consumeLoginEntryDiscovery() {
  const result = initial;
  initial = undefined;
  return result ?? fetchLoginEntries();
}

export function discardLoginEntryDiscovery() {
  initial = undefined;
}
