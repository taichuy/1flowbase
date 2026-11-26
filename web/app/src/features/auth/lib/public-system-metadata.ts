import { useEffect, useState } from 'react';
import { fetchPublicSystemMetadata } from '@1flowbase/api-client/auth';
import { setSeededPublicSystemMetadata } from '../../../shared/api/system-metadata-seed';
import { findDocumentArtwork } from '../components/hero/document-motion';

type Metadata = Awaited<ReturnType<typeof fetchPublicSystemMetadata>>;
let pending: Promise<Metadata> | undefined;
function metadataRequest() {
  // Only deduplicate a live request (including StrictMode's repeated render),
  // never a time-based business cache. The document owns its one-shot handoff.
  pending ??= fetchPublicSystemMetadata().finally(() => {
    pending = undefined;
  });
  void pending.catch(() => undefined);
  return pending;
}
export function usePublicSystemMetadata() {
  const [source] = useState(() => {
    const art = findDocumentArtwork();
    if (art?.metadata) {
      setSeededPublicSystemMetadata(art.metadata);
    }
    return {
      data: art?.metadata,
      flight: art?.metadata
        ? undefined
        : (art?.metadataFlight ?? metadataRequest())
    };
  });
  const [data, setData] = useState(source.data);
  useEffect(() => {
    let active = true;
    void source.flight?.then(
      (metadata) => {
        setSeededPublicSystemMetadata(metadata);
        if (active) setData(metadata);
      },
      () => undefined
    );
    return () => {
      active = false;
    };
  }, [source]);
  return { data };
}
