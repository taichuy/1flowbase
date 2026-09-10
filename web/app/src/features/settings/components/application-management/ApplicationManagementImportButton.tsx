import { ApiClientError } from '@1flowbase/api-client';
import ImportOutlined from '@ant-design/icons/es/icons/ImportOutlined';
import { useMutation } from '@tanstack/react-query';
import { App, Button } from 'antd';
import { useRef, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import {
  importApplicationArchive,
  previewApplicationArchive
} from '../../../applications/api/applications';
import { ApplicationArchiveImportModal } from '../../../applications/components/archive/ApplicationArchiveImportModal';

export function ApplicationManagementImportButton({
  csrfToken,
  onImported
}: {
  csrfToken: string;
  onImported: () => Promise<void>;
}) {
  const { message } = App.useApp();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [archive, setArchive] = useState<File | null>(null);
  const [names, setNames] = useState<Record<number, string>>({});
  const previewMutation = useMutation({
    mutationFn: previewApplicationArchive,
    onSuccess: (preview) => {
      setNames(
        Object.fromEntries(
          preview.applications.map((entry) => [
            entry.entry_index,
            entry.preview.application.name
          ])
        )
      );
      importMutation.reset();
    },
    onError: (error) => {
      setArchive(null);
      const code = error instanceof ApiClientError ? error.code : null;
      message.error(
        code === 'application_archive_application_count'
          ? i18nText('applications', 'archive_import.invalid_count')
          : code?.startsWith('application_archive')
            ? i18nText('applications', 'archive_import.invalid_archive')
            : i18nText('applications', 'auto.template_preview_failed')
      );
    }
  });
  const importMutation = useMutation({
    mutationFn: (file: File) =>
      importApplicationArchive(
        file,
        {
          applications: previewMutation.data?.applications.map(
            ({ entry_index }) => ({
              entry_index,
              name: names[entry_index].trim()
            })
          )
        },
        csrfToken
      ),
    onSuccess: async (results) => {
      await onImported();
      if (results.failed_count === 0 && results.partial_count === 0) {
        setArchive(null);
        previewMutation.reset();
        message.success(i18nText('applications', 'auto.template_imported'));
      }
    },
    onError: () => {
      message.error(i18nText('applications', 'auto.template_import_failed'));
    }
  });

  return (
    <>
      <input
        ref={fileInputRef}
        type="file"
        accept="application/zip,.zip,application/json,.json"
        aria-label={i18nText('applications', 'auto.import_template_file')}
        style={{ display: 'none' }}
        onChange={(event) => {
          const file = event.target.files?.[0];
          event.target.value = '';
          if (file) {
            setArchive(file);
            previewMutation.mutate(file);
          }
        }}
      />
      <Button
        aria-label={i18nText('applications', 'auto.import_template')}
        icon={<ImportOutlined />}
        loading={previewMutation.isPending}
        disabled={importMutation.isPending}
        onClick={() => fileInputRef.current?.click()}
      >
        {i18nText('applications', 'auto.import_template')}
      </Button>
      <ApplicationArchiveImportModal
        open={Boolean(archive && previewMutation.isSuccess)}
        preview={previewMutation.data ?? null}
        names={names}
        results={importMutation.data}
        importing={importMutation.isPending}
        onNameChange={(entry_index, name) =>
          setNames((current) => ({ ...current, [entry_index]: name }))
        }
        onCancel={() => {
          if (importMutation.isPending) return;
          setArchive(null);
          previewMutation.reset();
        }}
        onImport={() => {
          if (archive && !importMutation.isPending)
            importMutation.mutate(archive);
        }}
      />
    </>
  );
}
