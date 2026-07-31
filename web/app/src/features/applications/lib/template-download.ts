import type { ApiBlobResponse } from '@1flowbase/api-client';

export function downloadApplicationArchive(response: ApiBlobResponse) {
  const url = window.URL.createObjectURL(response.blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = response.filename ?? 'applications.zip';
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  window.URL.revokeObjectURL(url);
}
