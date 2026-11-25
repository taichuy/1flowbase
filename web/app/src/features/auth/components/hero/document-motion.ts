import type { fetchPublicSystemMetadata } from '@1flowbase/api-client/auth';
// The document owns motion until the routed Hero adopts it, or another route wins.
export interface DocumentArtwork extends HTMLDivElement {
  metadataFlight?: ReturnType<typeof fetchPublicSystemMetadata>;
  metadata?: Awaited<ReturnType<typeof fetchPublicSystemMetadata>>;
  stopMotion?: () => void;
}
export const documentSceneId = 'sign-in-document-scene';
export function findDocumentArtwork() {
  return document.querySelector<DocumentArtwork>(
    `#${documentSceneId} [data-hero-artwork]`
  );
}
export function discardDocumentScene() {
  findDocumentArtwork()?.stopMotion?.();
  document.getElementById(documentSceneId)?.remove();
}

export function setDocumentBrand(
  art: HTMLElement,
  slogan: string | null,
  title = '1flowbase'
) {
  const brand = art.querySelector<HTMLElement>('[data-opening-brand]')!;
  if (slogan !== null) {
    const titleLine = brand.querySelector('.hero-title-line');
    if (titleLine && titleLine.textContent !== title) {
      titleLine.textContent = title;
    }
    const line = brand.querySelector('.hero-slogan-line')!;
    if (line.textContent !== slogan) line.textContent = slogan;
  }
  brand.hidden = slogan === null;
}
