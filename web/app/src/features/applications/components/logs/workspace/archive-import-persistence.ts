export type PersistedRunArchiveImportJob = {
  jobId: string;
  fileName: string;
};

function archiveImportStorageKey(applicationId: string) {
  return `1flowbase.application.${applicationId}.run_archive_import_job`;
}

export function readPersistedArchiveImportJob(
  applicationId: string
): PersistedRunArchiveImportJob | null {
  if (typeof window === 'undefined') {
    return null;
  }

  const storedText = window.localStorage.getItem(
    archiveImportStorageKey(applicationId)
  );
  if (!storedText) {
    return null;
  }

  try {
    const parsedValue: unknown = JSON.parse(storedText);
    if (
      typeof parsedValue === 'object' &&
      parsedValue !== null &&
      typeof (parsedValue as PersistedRunArchiveImportJob).jobId === 'string' &&
      typeof (parsedValue as PersistedRunArchiveImportJob).fileName === 'string'
    ) {
      return parsedValue as PersistedRunArchiveImportJob;
    }
  } catch {
    window.localStorage.removeItem(archiveImportStorageKey(applicationId));
  }

  return null;
}

export function writePersistedArchiveImportJob(
  applicationId: string,
  job: PersistedRunArchiveImportJob
) {
  if (typeof window === 'undefined') {
    return;
  }

  window.localStorage.setItem(
    archiveImportStorageKey(applicationId),
    JSON.stringify(job)
  );
}

export function clearPersistedArchiveImportJob(applicationId: string) {
  if (typeof window === 'undefined') {
    return;
  }

  window.localStorage.removeItem(archiveImportStorageKey(applicationId));
}
