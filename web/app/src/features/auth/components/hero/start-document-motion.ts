import { startDocumentAuthDiscovery } from '../../api/document-start-discovery';
import { fetchPublicSystemMetadata } from '@1flowbase/api-client/auth';
import { getInitialAppLocale } from '../../../../shared/i18n/initial-locale';
import {
  enFooter,
  enTagline,
  zhFooter,
  zhTagline
} from 'virtual:1flowbase-opening-copy';
import {
  findDocumentArtwork,
  setDocumentBrand,
  documentSceneId
} from './document-motion';
import { startLeafMotion } from './leaf-motion';
// Start API work in the HTML discovery wave, independent of React and animation.
startDocumentAuthDiscovery();
const art = findDocumentArtwork();
if (art) {
  art.stopMotion = startLeafMotion(
    art.querySelector('canvas')!,
    art.querySelector('.auth-sign-in-hero')!
  );

  const chinese = getInitialAppLocale() === 'zh_Hans';
  const slogan = chinese ? zhTagline : enTagline;
  const scene = document.getElementById(documentSceneId)!;
  scene.querySelector('.auth-sign-in-footer a')!.textContent = chinese
    ? zhFooter
    : enFooter;
  // The public response, not the core bundle or auth decision, owns this reveal.
  art.metadataFlight = fetchPublicSystemMetadata();
  void art.metadataFlight
    .then((metadata) => {
      art.metadata = metadata;
      if (!scene.isConnected) return;
      setDocumentBrand(
        art,
        metadata.hide_login_tagline === false ? slogan : null
      );
      if (metadata.filing_html) {
        const filing = scene.querySelector<HTMLElement>(
          '[data-opening-filing]'
        )!;
        filing.className = 'auth-sign-in-filing';
        filing.innerHTML = metadata.filing_html;
        filing.hidden = false;
      }
    })
    .catch(() => {
      delete art.metadataFlight;
    });
}
