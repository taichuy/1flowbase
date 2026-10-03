import type {
  PortableTemplateArchive,
  PortableTemplatePackage
} from '@1flowbase/api-client';

export async function readTemplateFile(
  file: File
): Promise<PortableTemplatePackage> {
  if (!file.name.toLowerCase().endsWith('.zip')) {
    return JSON.parse(await file.text());
  }
  // Archive parsing, integrity checks and schema validation belong to the server.
  return new Promise<PortableTemplateArchive>((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error);
    reader.onabort = () => reject(new Error('Template file read aborted'));
    reader.onload = () => {
      const dataUrl = String(reader.result);
      resolve({ archive_base64: dataUrl.slice(dataUrl.indexOf(',') + 1) });
    };
    reader.readAsDataURL(file);
  });
}

export function templateArchiveBlob(archive_base64: string): Blob {
  const bytes = Uint8Array.from(atob(archive_base64), (byte) =>
    byte.charCodeAt(0)
  );
  return new Blob([bytes], { type: 'application/zip' });
}
