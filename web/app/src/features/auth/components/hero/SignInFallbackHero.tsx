import { useLayoutEffect, useRef } from 'react';
import { HeroArtwork, HeroTagline } from './HeroArtwork';
import type { DocumentArtwork } from './document-motion';
import { startLeafMotion } from './leaf-motion';

// Only the no-opening and missing-motion recovery paths load this module.
export default function SignInFallbackHero({
  opening,
  slogan
}: {
  opening: DocumentArtwork | null;
  slogan: string | null;
}) {
  const host = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (opening) {
      // HeroAnimation owns the adopted canvas lifetime, including the deferred
      // StrictMode cleanup. Never restart motion that recovered during loading.
      opening.stopMotion ??= startLeafMotion(
        opening.querySelector('canvas')!,
        opening.querySelector('.auth-sign-in-hero')!
      );
      return;
    }
    const target = host.current!;
    return startLeafMotion(
      target.querySelector('canvas')!,
      target.querySelector('.auth-sign-in-hero')!
    );
  }, [opening]);
  return opening ? null : (
    <div ref={host} style={{ display: 'contents' }}>
      <HeroArtwork>
        {slogan !== null && <HeroTagline slogan={slogan} />}
      </HeroArtwork>
    </div>
  );
}
