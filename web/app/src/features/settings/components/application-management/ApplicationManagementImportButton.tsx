import ImportOutlined from '@ant-design/icons/es/icons/ImportOutlined';
import { useMutation } from '@tanstack/react-query';
import { App, Button } from 'antd';
import { useRef, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import {
  importApplicationArchive,
  previewApplicationArchive
} from '../../../applications/api/applications';
import { ApplicationTemplateImportModal } from '../../../applications/components/ApplicationTemplateImportModal';

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
  const [name, setName] = useState('');
  const previewMutation = useMutation({
    mutationFn: previewApplicationArchive,
    onSuccess: (preview) => setName(preview.application.name),
    onError: () => {
      setArchive(null);
      message.error(i18nText('applications', 'auto.template_preview_failed'));
    }
  });
  const importMutation = useMutation({
    mutationFn: (file: File) =>
      importApplicationArchive(
        file,
        {
          name: name.trim(),
          description: previewMutation.data?.application.description
        },
        csrfToken
      ),
    onSuccess: async () => {
      setArchive(null);
      previewMutation.reset();
      message.success(i18nText('applications', 'auto.template_imported'));
      await onImported();
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
      <ApplicationTemplateImportModal
        open={Boolean(archive && previewMutation.isSuccess)}
        preview={previewMutation.data ?? null}
        name={name}
        importing={importMutation.isPending}
        onNameChange={setName}
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
