import type { ApplicationRunExportDownload } from '../api/runtime';

function normalizeDownloadName(value: string) {
  return (
    value
      .trim()
      .replace(/[^\p{Letter}\p{Number}._-]+/gu, '-')
      .replace(/^-+|-+$/g, '') || 'application-run'
  );
}

export function buildRunTraceDumpFilename(runId: string) {
  return `${normalizeDownloadName(runId)}-trace.json`;
}

export function buildSelectedRunTraceDumpFilename() {
  return 'application-run-traces.zip';
}

export function saveApplicationRunExport(
  download: ApplicationRunExportDownload,
  fallbackFilename: string
) {
  const url = window.URL.createObjectURL(download.blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = download.filename ?? fallbackFilename;
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  window.URL.revokeObjectURL(url);
}
