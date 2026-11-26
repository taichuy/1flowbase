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
  const footer = scene.querySelector<HTMLElement>('.auth-sign-in-footer')!;
  footer.querySelector('a')!.textContent = chinese ? zhFooter : enFooter;
  // The public response, not the core bundle or auth decision, owns this reveal.
  art.metadataFlight = fetchPublicSystemMetadata();
  void art.metadataFlight
    .then((metadata) => {
      art.metadata = metadata;
      const nextTitle =
        metadata.page_title?.trim() || metadata.system_name?.trim();
      if (nextTitle) document.title = nextTitle;
      if (!scene.isConnected) return;
      setDocumentBrand(
        art,
        metadata.hide_login_tagline === false
          ? (metadata.login_hero_tagline?.trim() || slogan)
          : null,
        metadata.login_hero_title?.trim() || '1flowbase'
      );
      footer.hidden = Boolean(metadata.hide_powered_by);
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
