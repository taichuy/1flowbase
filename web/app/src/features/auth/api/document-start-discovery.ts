import { fetchCurrentSession, fetchLoginEntries } from './session';

interface DocumentDiscovery {
  session?: ReturnType<typeof fetchCurrentSession>;
  entries?: ReturnType<typeof fetchLoginEntries>;
}

// The inline IIFE and the application ESM are different module instances. The
// document, not a module cache or TTL, owns this single navigation's handoff.
const handoffKey = '__oneflowbaseSignInDiscovery';
type DiscoveryWindow = Window & {
  [handoffKey]?: DocumentDiscovery;
};

function owner(): DiscoveryWindow | undefined {
  return typeof window === 'undefined' ? undefined : window;
}

export function startDocumentAuthDiscovery() {
  const target = owner();
  if (!target || !/^(?:\/|\/sign-in\/?)$/.test(target.location.pathname)) return;
  if (target[handoffKey]) return;
  const handoff: DocumentDiscovery = {
    session: fetchCurrentSession(),
    entries: fetchLoginEntries()
  };
  target[handoffKey] = handoff;
  // The later consumer owns both results/errors; a normal anonymous 401 must
  // not become an unhandled rejection while application JS is downloading.
  void handoff.session?.catch(() => undefined);
  void handoff.entries?.catch(() => undefined);
}

export function takeDocumentSessionDiscovery() {
  const handoff = owner()?.[handoffKey];
  const result = handoff?.session;
  if (handoff) delete handoff.session;
  return result;
}

export function takeDocumentLoginEntryDiscovery() {
  const handoff = owner()?.[handoffKey];
  const result = handoff?.entries;
  if (handoff) delete handoff.entries;
  return result;
}

export function discardDocumentSessionDiscovery() {
  const handoff = owner()?.[handoffKey];
  if (handoff) delete handoff.session;
}

export function discardDocumentLoginEntryDiscovery() {
  const handoff = owner()?.[handoffKey];
  if (handoff) delete handoff.entries;
}
