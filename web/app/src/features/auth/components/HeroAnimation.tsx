import { useLayoutEffect, useRef, useState } from 'react';
import { i18nText } from '../../../shared/i18n/text';
import { HeroArtwork, HeroTagline } from './hero/HeroArtwork';
import {
  findDocumentArtwork,
  discardDocumentScene,
  setDocumentBrand
} from './hero/document-motion';
import { startLeafMotion } from './hero/leaf-motion';

export function HeroAnimation({
  hide_login_tagline
}: {
  hide_login_tagline?: boolean;
}) {
  const host = useRef<HTMLDivElement>(null);
  const [opening] = useState(findDocumentArtwork);
  const slogan = hide_login_tagline
    ? null
    : i18nText('auth', 'auto.product_tagline');
  useLayoutEffect(() => {
    if (opening) setDocumentBrand(opening, slogan);
  }, [opening, slogan]);
  useLayoutEffect(() => {
    const target = host.current!;
    if (opening) {
      // Move the live canvas, its pointer target and its simulation together. Do not restart on handoff.
      target.appendChild(opening);
      opening.stopMotion ??= startLeafMotion(
        opening.querySelector('canvas')!,
        opening.querySelector('.auth-sign-in-hero')!
      );
      discardDocumentScene();
      return () => {
        queueMicrotask(() => {
          if (!opening.isConnected) {
            opening.stopMotion?.();
            delete opening.stopMotion;
          }
        });
      };
    }
    return startLeafMotion(
      target.querySelector('canvas')!,
      target.querySelector('.auth-sign-in-hero')!
    );
  }, [opening]);
  return (
    <div ref={host} style={{ display: 'contents' }}>
      {!opening && (
        <HeroArtwork>
          {slogan !== null && <HeroTagline slogan={slogan} />}
        </HeroArtwork>
      )}
    </div>
  );
}
