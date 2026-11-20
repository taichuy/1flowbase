import { StrictMode } from 'react';
import { act, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import { fetchPublicSystemMetadata } from '@1flowbase/api-client/auth';
import { usePublicSystemMetadata } from '../public-system-metadata';
import type { DocumentArtwork } from '../../components/hero/document-motion';

vi.mock('@1flowbase/api-client/auth', () => ({
  fetchPublicSystemMetadata: vi.fn()
}));
const metadata = {
  system_name: 'Document brand',
  system_description: '',
  logo_url: '',
  hide_login_tagline: false,
  filing_html: '<b>filing</b>'
};
function Probe() {
  const { data } = usePublicSystemMetadata();
  return <p>{data?.system_name ?? 'waiting'}</p>;
}
function artwork() {
  const scene = document.createElement('div');
  scene.id = 'sign-in-document-scene';
  const art = document.createElement('div') as DocumentArtwork;
  art.dataset.heroArtwork = '';
  scene.append(art);
  document.body.append(scene);
  return { scene, art };
}
beforeEach(() => {
  vi.mocked(fetchPublicSystemMetadata).mockReset();
  document.getElementById('sign-in-document-scene')?.remove();
});
test('consumes settled metadata with no second fetch', () => {
  const { art, scene } = artwork();
  art.metadata = metadata;
  render(
    <StrictMode>
      <Probe />
    </StrictMode>
  );
  expect(screen.getByText('Document brand')).toBeVisible();
  expect(fetchPublicSystemMetadata).not.toHaveBeenCalled();
  scene.remove();
});
test('keeps the document promise after scene handoff/removal and StrictMode effect replay', async () => {
  const { art, scene } = artwork();
  let resolve!: (data: typeof metadata) => void;
  art.metadataFlight = new Promise((done) => {
    resolve = done;
  });
  render(
    <StrictMode>
      <Probe />
    </StrictMode>
  );
  scene.remove();
  await act(async () => resolve(metadata));
  expect(screen.getByText('Document brand')).toBeVisible();
  expect(fetchPublicSystemMetadata).not.toHaveBeenCalled();
});
test('deduplicates an active fallback request but does not cache a later visit', async () => {
  vi.mocked(fetchPublicSystemMetadata).mockResolvedValue(metadata);
  const view = render(
    <StrictMode>
      <Probe />
    </StrictMode>
  );
  expect(await screen.findByText('Document brand')).toBeVisible();
  expect(fetchPublicSystemMetadata).toHaveBeenCalledTimes(1);
  view.unmount();
  render(<Probe />);
  await waitFor(() =>
    expect(fetchPublicSystemMetadata).toHaveBeenCalledTimes(2)
  );
});
test('metadata failure keeps the safe hidden-brand/no-filing state and permits a new visit', async () => {
  vi.mocked(fetchPublicSystemMetadata)
    .mockRejectedValueOnce(new Error('offline'))
    .mockResolvedValueOnce(metadata);
  const view = render(<Probe />);
  await act(async () => {
    await Promise.resolve();
  });
  expect(screen.getByText('waiting')).toBeVisible();
  view.unmount();
  render(<Probe />);
  expect(await screen.findByText('Document brand')).toBeVisible();
});
