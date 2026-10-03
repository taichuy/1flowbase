import { useLayoutEffect, useState } from 'react';
import InfiniteScroll, { type Props } from 'react-infinite-scroll-component';

import { useNativeBlockSurface } from '../native-block-surface-context';

// Resolve IDs after the parent DOM commits, within this Block's ShadowRoot.
// The upstream component otherwise searches document and falls back to window.
export default function NativeInfiniteScroll(props: Props) {
  const surface = useNativeBlockSurface();
  const [resolved, setResolved] = useState<{
    source: string;
    root: ShadowRoot;
    element: HTMLElement;
  } | null>(null);
  const source = props.height ? undefined : props.scrollableTarget;

  useLayoutEffect(() => {
    if (!surface || typeof source !== 'string') return;
    const element = surface.targetRoot.getElementById(source);
    if (!(element instanceof HTMLElement)) {
      throw new Error(
        `InfiniteScroll target '${source}' was not found in this Block.`
      );
    }
    setResolved({ source, root: surface.targetRoot, element });
  }, [source, surface]);

  if (!surface)
    throw new Error('InfiniteScroll requires a native Block surface.');
  if (typeof source === 'string') {
    if (resolved?.source !== source || resolved.root !== surface.targetRoot)
      return null;
    return <InfiniteScroll {...props} scrollableTarget={resolved.element} />;
  }
  const target =
    source ??
    (surface.scrollOwner instanceof HTMLElement
      ? surface.scrollOwner
      : undefined);
  return <InfiniteScroll {...props} scrollableTarget={target} />;
}
