import { useCallback, useEffect, useRef, useState } from 'react';
import { Alert, ConfigProvider } from 'antd';
import { FormBlockInitializer } from './FormBlockInitializer';
import {
  BLOCK_DESIGN_THEME,
  BLOCK_BUTTON_THEME
} from '../../../lib/native-modules/block-design/theme';
import { useDataBlockPopupContainer } from '../shared/useDataBlockPopupContainer';
import { i18nText } from '../../../../../shared/i18n/text';
import { dataBlockError } from '../../../lib/data-blocks/fields';
import { useDataBlockHost } from '../../../lib/data-blocks/host-context';
import {
  readDataBlockConfig,
  type DataBlockComponentProps,
  type FormBlockConfig
} from '../../../lib/data-blocks/types';
import { type DesignDialog } from './FormDesignControls';
import { recordIdentity } from './form-state';
import { useFormRecordSession } from './use-form-record-session';
import { FormBlockView } from './FormBlockView';

export function FormBlock({ ctx }: DataBlockComponentProps) {
  const incoming = readDataBlockConfig(ctx.props);
  const [initialization, setInitialization] = useState<{
    source: FormBlockConfig;
    value: FormBlockConfig;
  } | null>(null);
  const config =
    initialization && initialization.source === incoming
      ? initialization.value
      : incoming;
  const host = useDataBlockHost();
  const popupContainer = useDataBlockPopupContainer();
  if (!config || config.kind !== 'form')
    return (
      <Alert
        type="warning"
        showIcon
        title={i18nText('frontstage', 'data_blocks.form.not_configured')}
      />
    );
  return (
    <ConfigProvider
      theme={host?.design_mode ? BLOCK_DESIGN_THEME : BLOCK_BUTTON_THEME}
      getPopupContainer={popupContainer}
    >
      {config.initialized === false ? (
        <FormBlockInitializer
          ctx={ctx}
          config={config}
          onInitialized={(value) =>
            setInitialization({ source: config, value })
          }
        />
      ) : (
        <FormBlockSession
          key={`${ctx.workspace.id}:${ctx.currentUser?.id ?? ''}:${host?.block_id ?? ''}:${config.binding.model_id}:${recordIdentity(config, ctx.inputs)}`}
          ctx={ctx}
          initialConfig={config}
        />
      )}
    </ConfigProvider>
  );
}

function FormBlockSession({
  ctx,
  initialConfig
}: DataBlockComponentProps & { initialConfig: FormBlockConfig }) {
  const host = useDataBlockHost();
  const source = useRef({ config: initialConfig, generation: 0 });
  if (source.current.config !== initialConfig) {
    source.current = {
      config: initialConfig,
      generation: source.current.generation + 1
    };
  }
  const generation = source.current.generation;
  const [designState, setDesignState] = useState<{
    generation: number;
    config: FormBlockConfig;
    error: string;
  } | null>(null);
  // A replacement host source wins during render, while record drafts keep their session.
  const currentDesign =
    designState?.generation === generation ? designState : null;
  const config = currentDesign?.config ?? initialConfig;
  const [savingDesign, setSavingDesign] = useState(false);
  const [dialog, setDialog] = useState<DesignDialog>(null);
  const active = useRef(true);
  const saving = useRef(false);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  const recordSession = useFormRecordSession(ctx, config, host, active);
  const saveConfig = useCallback(
    async (next: FormBlockConfig) => {
      if (
        !host ||
        !host.design_mode ||
        saving.current ||
        source.current.generation !== generation
      )
        return false;
      const previous = config;
      saving.current = true;
      setSavingDesign(true);
      setDesignState({ generation, config: next, error: '' });
      try {
        await host.saveConfig(next);
        return active.current && source.current.generation === generation;
      } catch (error) {
        if (active.current && source.current.generation === generation) {
          setDesignState({
            generation,
            config: previous,
            error: dataBlockError(error)
          });
        }
        return false;
      } finally {
        saving.current = false;
        if (active.current) setSavingDesign(false);
      }
    },
    [config, generation, host]
  );
  return (
    <FormBlockView
      ctx={ctx}
      config={config}
      host={host}
      session={recordSession}
      designError={currentDesign?.error ?? ''}
      savingDesign={savingDesign}
      saveConfig={saveConfig}
      dialog={dialog}
      setDialog={setDialog}
    />
  );
}
