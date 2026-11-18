import { useState } from 'react';
import { Button, Popover, Space, Typography } from 'antd';
import { i18nText } from '../../../../../../shared/i18n/text';
import { emptyDataFilter } from '../../../../lib/data-blocks/types';
import {
  restrictDataFilter,
  validDataFilter
} from '../../../../lib/data-blocks/filter-variables';
import { DataBlockFilterEditor } from '../../shared/DataBlockFilterEditor';
import { tableActionAllowed } from '../table-model';
import { TableActionMenu } from '../TableDesignControls';
import { TableAction } from './TableAction';
import type { useTableConfigController } from '../use-table-config-controller';
import type { useTableSession } from '../runtime/use-table-session';
import type { useTableActions } from '../runtime/use-table-actions';

export function TableToolbar({
  configuration,
  session,
  actions,
  disabled,
  design
}: {
  configuration: ReturnType<typeof useTableConfigController>;
  session: ReturnType<typeof useTableSession>;
  actions: ReturnType<typeof useTableActions>;
  disabled: boolean;
  design: boolean;
}) {
  const { config, saveAction } = configuration;
  const { fields, variables, capabilities, filter } = session;
  const [filterDraft, setFilterDraft] = useState(emptyDataFilter);
  const [filterOpen, setFilterOpen] = useState(false);
  const [filterSaved, setFilterSaved] = useState(false);
  const filterActions = config.actions.filter(
    (action) => action.enabled && action.kind === 'filter'
  );
  const filterAction = filterActions[0];
  const filterableFields = fields.filter(
    (field) =>
      !filterAction?.filter_fields ||
      filterAction.filter_fields.includes(field.code)
  );
  const filterValid = validDataFilter(filterDraft, filterableFields, variables);
  const filterContent = (
    <div className="data-table-filter-panel">
      <DataBlockFilterEditor
        value={filterDraft}
        fields={filterableFields}
        variables={variables}
        onChange={(value) => {
          setFilterDraft(value);
          setFilterSaved(false);
        }}
      />
      {filterSaved && (
        <Typography.Text type="success">
          {i18nText('frontstage', 'data_blocks.filter.conditions_saved')}
        </Typography.Text>
      )}
      {!filterValid && (
        <Typography.Text type="secondary">
          {i18nText('frontstage', 'data_blocks.filter.incomplete')}
        </Typography.Text>
      )}
      <div className="data-table-filter-buttons">
        {design && filterAction && (
          <Button
            size="small"
            type="dashed"
            disabled={disabled || !filterValid}
            onClick={async () => {
              if (
                await saveAction(
                  { ...filterAction, default_filter: filterDraft },
                  false
                )
              ) {
                session.applyFilter(filterDraft);
                setFilterSaved(true);
              }
            }}
          >
            {i18nText('frontstage', 'data_blocks.filter.save_conditions')}
          </Button>
        )}
        <Button
          size="small"
          onClick={() => {
            const empty = filterAction?.default_filter ?? emptyDataFilter();
            setFilterDraft(empty);
            session.applyFilter(empty);
          }}
        >
          {i18nText('frontstage', 'data_blocks.table.reset')}
        </Button>
        <Button
          size="small"
          type="primary"
          disabled={!filterValid}
          onClick={() => {
            session.applyFilter(filterDraft);
            setFilterOpen(false);
          }}
        >
          {i18nText('frontstage', 'data_blocks.table.apply')}
        </Button>
      </div>
    </div>
  );
  return (
    <div className="data-table-toolbar">
      <div>
        {filterActions.map(
          (action) =>
            (design || tableActionAllowed(action, capabilities)) && (
              <Popover
                key={action.id}
                trigger="click"
                placement="bottomLeft"
                open={filterOpen}
                onOpenChange={(open) => {
                  if (open)
                    setFilterDraft(
                      restrictDataFilter(
                        filter,
                        filterableFields.map((field) => field.code)
                      )
                    );
                  setFilterSaved(false);
                  setFilterOpen(open);
                }}
                content={filterContent}
              >
                <TableAction
                  action={action}
                  session={session}
                  actions={actions}
                  configuration={configuration}
                  disabled={disabled}
                  design={design}
                  onFilter={() => {
                    setFilterDraft(filter);
                    setFilterOpen(true);
                  }}
                />
              </Popover>
            )
        )}
      </div>
      <Space size="small" wrap>
        {config.actions.map((action) =>
          action.enabled && action.kind !== 'filter' ? (
            <TableAction
              key={action.id}
              action={action}
              session={session}
              actions={actions}
              configuration={configuration}
              disabled={disabled}
              design={design}
            />
          ) : null
        )}
        {design && (
          <TableActionMenu
            actions={config.actions}
            kinds={['filter', 'create', 'delete', 'refresh']}
            disabled={disabled}
            label={i18nText('frontstage', 'data_blocks.form.configure_actions')}
            onChange={(action) => saveAction(action, false)}
          />
        )}
      </Space>
    </div>
  );
}
