import { useRef, useState } from 'react';
import { Alert } from 'antd';
import type { BlockContext } from '@1flowbase/page-protocol';
import { i18nText } from '../../../../../shared/i18n/text';
import { useFrontstageBlockCatalog } from '../../../hooks/use-frontstage-block-catalog';
import { useDataBlockHost } from '../../../lib/data-blocks/host-context';
import { dataBlockError } from '../../../lib/data-blocks/fields';
import {
  FORM_BLOCK_CONTRIBUTION,
  type DataBlockBinding,
  type FormBlockConfig
} from '../../../lib/data-blocks/types';
import { CreateDataBlockMenu } from '../CreateDataBlockMenu';

export function FormBlockInitializer({
  ctx,
  config,
  onInitialized
}: {
  ctx: BlockContext;
  config: FormBlockConfig;
  onInitialized(config: FormBlockConfig): void;
}) {
  const host = useDataBlockHost();
  const catalog = useFrontstageBlockCatalog({
    workspaceId: host?.design_mode ? ctx.workspace.id : null
  });
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  const [error, setError] = useState('');
  if (!host?.design_mode)
    return (
      <Alert
        type="info"
        showIcon
        title={i18nText('frontstage', 'data_blocks.form.uninitialized')}
      />
    );
  const initialize = async (binding: DataBlockBinding | undefined) => {
    if (!binding || savingRef.current || host.saving) return;
    const sameBinding =
      binding.model_id === config.binding.model_id &&
      binding.model_code === config.binding.model_code &&
      binding.data_source_id === config.binding.data_source_id;
    if (!sameBinding && config.mode !== 'create') return;
    const next: FormBlockConfig = {
      ...config,
      binding,
      initialized: true,
      ...(!sameBinding ? { items: [], rules: [], relation: undefined } : {})
    };
    savingRef.current = true;
    setSaving(true);
    setError('');
    try {
      await host.saveConfig(next);
      onInitialized(next);
    } catch (failure) {
      setError(dataBlockError(failure));
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  };
  return (
    <section className="data-form-block">
      {error && (
        <Alert
          type="error"
          showIcon
          title={i18nText('frontstage', 'data_blocks.form.design_failed')}
          description={error}
        />
      )}
      <CreateDataBlockMenu
        workspaceId={ctx.workspace.id}
        entries={catalog.items.filter(
          (entry) => entry.contributionCode === FORM_BLOCK_CONTRIBUTION
        )}
        currentBinding={config.binding}
        allowOtherModels={config.mode === 'create'}
        disabled={saving || host.saving || catalog.loading}
        loading={saving || catalog.loading}
        error={catalog.error}
        onRetry={() => {
          void catalog.refetch();
        }}
        onCreate={(_entry, binding) => initialize(binding)}
      />
    </section>
  );
}
