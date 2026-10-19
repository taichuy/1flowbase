import { useContext, useEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  App,
  Button,
  ConfigProvider,
  Input,
  Tag,
  Modal,
  Space,
  Tooltip,
  Upload
} from 'antd';
import type { UploadFile, UploadProps } from 'antd';
import CopyOutlined from '@ant-design/icons/es/icons/CopyOutlined';
import copy from 'copy-to-clipboard';
import SearchOutlined from '@ant-design/icons/es/icons/SearchOutlined';
import FileTextOutlined from '@ant-design/icons/es/icons/FileTextOutlined';
import { FileRecordActions } from './FileRecordActions';
import './file-table-view.css';
import InboxOutlined from '@ant-design/icons/es/icons/InboxOutlined';
import UploadOutlined from '@ant-design/icons/es/icons/UploadOutlined';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import {
  fetchConsoleFileRecords,
  fetchConsoleFileTables,
  uploadConsoleFile,
  type ConsoleFileRecord
} from '@1flowbase/api-client';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import { formatDateTime, formatNumber } from '../../../../shared/i18n/format';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { DataTableLayout } from '../../../../shared/ui/data-table/DataTableLayout';
import { usePersistedDataTableConfiguration } from '../../../../shared/ui/data-table/data-table-state';
import { useNativeBlockSurface } from '../../lib/native-modules/native-block-surface-context';
import { FileShareModal } from './FileShareModal';
import { FileRecordPreview } from './FileRecordPreview';
import { formatFileSize } from './file-size';

export interface FileTableViewProps {
  fileTableCode: string;
  toolbarExtra?: ReactNode;
}

/** Host owns authenticated multipart requests; Blocks only select a file table. */
function useFileTableController({ fileTableCode, toolbarExtra }: FileTableViewProps) {
  const { message } = App.useApp();
  const { getPrefixCls } = useContext(ConfigProvider.ConfigContext);
  const uploadPrefixCls = getPrefixCls('upload');
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id
  );
  const surface = useNativeBlockSurface();
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(5);
  const [search, setSearch] = useState('');
  const [filename, setFilename] = useState('');
  const [sort, setSort] = useState('created_at:desc');
  useEffect(() => {
    const timer = setTimeout(() => {
      setFilename(search.trim());
      setPage(1);
    }, 300);
    return () => clearTimeout(timer);
  }, [search]);
  const [uploadOpen, setUploadOpen] = useState(false);
  const [fileList, setFileList] = useState<UploadFile[]>([]);
  const [shareTarget, setShareTarget] = useState<{
    record: ConsoleFileRecord;
    tableId: string;
    workspaceId: string | undefined;
  }>();
  const [uploads] = useState(() => new Set<AbortController>());
  useEffect(() => {
    const controllers = uploads;
    return () => {
      for (const controller of controllers) controller.abort();
    };
  }, [workspaceId, fileTableCode]);

  const tables = useQuery({
    queryKey: ['files', 'tables', workspaceId],
    queryFn: () => fetchConsoleFileTables()
  });
  const table = tables.data?.find((item) => item.code === fileTableCode);
  const records = useQuery({
    queryKey: [
      'files',
      'records',
      workspaceId,
      table?.id,
      page,
      pageSize,
      filename,
      sort
    ],
    queryFn: ({ signal }) =>
      fetchConsoleFileRecords(fileTableCode, page, pageSize, signal, {
        filename,
        sort
      }),
    enabled: Boolean(table)
  });
  useEffect(() => {
    setPage(1);
    setShareTarget(undefined);
    setSearch('');
    setFilename('');
  }, [fileTableCode, workspaceId]);
  useEffect(() => setShareTarget(undefined), [csrfToken]);

  const columns = useMemo<DataTableColumn<ConsoleFileRecord>[]>(
    () => [
      {
        key: 'preview_url',
        dataIndex: 'preview_url',
        title: i18nText('frontstage', 'auto.preview'),
        width: 128,
        render: (_, record) => <FileRecordPreview record={record} />
      },
      {
        key: 'filename',
        dataIndex: 'filename',
        title: i18nText('frontstage', 'files.filename'),
        width: 280,
        render: (_, record) => (
          <div className="file-table-view__filename">
            <a
              href={record.url}
              target="_blank"
              rel="noreferrer"
              title={record.filename}
            >
              {record.filename}
            </a>
            <span>
              {record.mimetype.startsWith('image/')
                ? i18nText('frontstage', 'files.image_file')
                : i18nText('frontstage', 'files.other_file')}
            </span>
          </div>
        )
      },
      {
        key: 'size',
        dataIndex: 'size',
        title: i18nText('frontstage', 'files.size'),
        width: 124,
        sorter: true,
        sortOrder: sort.startsWith('size:')
          ? sort.endsWith('asc')
            ? 'ascend'
            : 'descend'
          : null,
        render: (_, record) => (
          <Tooltip
            title={`${formatNumber(record.size)} B`}
            getPopupContainer={surface?.overlayHost.getPopupContainer}
          >
            <span>{formatFileSize(record.size)}</span>
          </Tooltip>
        )
      },
      {
        key: 'mimetype',
        dataIndex: 'mimetype',
        title: i18nText('frontstage', 'files.mimetype'),
        width: 154,
        render: (_, record) => (
          <Tag className="file-table-view__type">{record.mimetype}</Tag>
        )
      },
      {
        key: 'url',
        dataIndex: 'url',
        title: i18nText('frontstage', 'files.url'),
        width: 330,
        render: (_, record) => (
          <div className="file-table-view__url">
            <a
              href={record.url}
              title={record.url}
              target="_blank"
              rel="noreferrer"
            >
              {record.url}
            </a>
            <Tooltip
              title={i18nText('frontstage', 'files.copy_link')}
              getPopupContainer={surface?.overlayHost.getPopupContainer}
            >
              <Button
                size="small"
                type="text"
                icon={<CopyOutlined aria-hidden="true" />}
                aria-label={i18nText('frontstage', 'files.copy_link')}
                onClick={async () =>
                  (await copy(record.url))
                    ? message.success(
                        i18nText('frontstage', 'files.link_copied')
                      )
                    : message.error(i18nText('frontstage', 'files.copy_failed'))
                }
              />
            </Tooltip>
          </div>
        )
      },
      {
        key: 'created_at',
        dataIndex: 'created_at',
        title: i18nText('frontstage', 'files.created_at'),
        width: 180,
        sorter: true,
        sortOrder: sort.startsWith('created_at:')
          ? sort.endsWith('asc')
            ? 'ascend'
            : 'descend'
          : null,
        render: (_, record) => formatDateTime(record.created_at)
      },
      {
        key: 'actions',
        title: i18nText('frontstage', 'files.actions'),
        width: 300,
        render: (_, record) => (
          <FileRecordActions
            key={`${workspaceId}:${fileTableCode}:${record.id}:${csrfToken}`}
            record={record}
            fileTableCode={fileTableCode}
            csrfToken={csrfToken}
            onShare={() =>
              table &&
              setShareTarget({ record, tableId: table.id, workspaceId })
            }
            onChanged={async () => {
              await queryClient.invalidateQueries({
                queryKey: ['files', 'records', workspaceId, table?.id]
              });
              setPage(1);
            }}
          />
        )
      }
    ],
    [
      table,
      csrfToken,
      workspaceId,
      surface,
      sort,
      fileTableCode,
      queryClient,
      message
    ]
  );
  const configuration = usePersistedDataTableConfiguration({
    columns,
    storageKey: `file-table:${workspaceId}:${fileTableCode}`
  });
  const uploading = fileList.some((file) => file.status === 'uploading');

  const customRequest: UploadProps['customRequest'] = (request) => {
    const controller = new AbortController();
    uploads.add(controller);
    async function upload() {
      if (!table || !csrfToken || !(request.file instanceof File)) {
        throw new Error(i18nText('frontstage', 'files.upload_unavailable'));
      }
      const result = await uploadConsoleFile(
        request.file,
        table.id,
        csrfToken,
        controller.signal
      );
      if (controller.signal.aborted) return;
      request.onSuccess?.(result);
      message.success(
        i18nText('frontstage', 'files.upload_success', {
          filename: request.file.name
        })
      );
      // Invalidate cached pages before returning to the newest records.
      await queryClient.invalidateQueries({
        queryKey: ['files', 'records', workspaceId, table.id]
      });
      setPage(1);
    }
    void upload()
      .catch((error: unknown) => {
        if (controller.signal.aborted) return;
        const failure =
          error instanceof Error
            ? error
            : new Error(i18nText('frontstage', 'files.upload_failed'));
        request.onError?.(failure);
        message.error(failure.message);
      })
      .finally(() => {
        uploads.delete(controller);
      });
    return { abort: () => controller.abort() };
  };

  const error = tables.error ?? records.error;
  return {
    error, message, tables, table, columns, configuration, records, page, pageSize,
    setPage, setPageSize, setSort, search, setSearch, csrfToken, setUploadOpen,
    toolbarExtra, shareTarget, workspaceId, filename, setShareTarget, uploadOpen,
    surface, uploading, setFileList, fileList, customRequest, uploadPrefixCls
  };
}

export function FileTableView(props: Parameters<typeof useFileTableController>[0]) {
  const {
    error, message, tables, table, columns, configuration, records, page, pageSize,
    setPage, setPageSize, setSort, search, setSearch, csrfToken, setUploadOpen,
    toolbarExtra, shareTarget, workspaceId, filename, setShareTarget, uploadOpen,
    surface, uploading, setFileList, fileList, customRequest, uploadPrefixCls
  } = useFileTableController(props);
  return (
    <div className="file-table-view">
      {error ? <Alert showIcon type="error" title={error.message} /> : null}
      {!tables.isPending && !tables.error && !table ? (
        <Alert
          showIcon
          type="warning"
          title={i18nText('frontstage', 'files.table_unavailable')}
        />
      ) : null}
      <DataTableLayout className="file-table-view__layout">
        <DataTable
          className="file-table-view__table"
          columns={columns}
          configuration={configuration}
          dataSource={records.data?.items ?? []}
          rowKey="id"
          loading={tables.isPending || records.isFetching}
          page={page}
          pageSize={pageSize}
          total={records.data?.total ?? 0}
          pageSizeOptions={[5, 10, 20, 50]}
          onPageChange={(next, size) => {
            setPage(size !== pageSize ? 1 : next);
            setPageSize(size);
          }}
          showTotal={(total, [start, end]) =>
            i18nText('frontstage', 'files.range', { start, end, total })
          }
          onChange={(_, __, sorter) => {
            const selected = Array.isArray(sorter) ? sorter[0] : sorter;
            setSort(
              selected?.order
                ? `${selected.columnKey}:${selected.order === 'ascend' ? 'asc' : 'desc'}`
                : 'created_at:desc'
            );
            setPage(1);
          }}
          emptyText={i18nText('frontstage', 'files.empty')}
          toolbar={
            <>
              <div className="file-table-view__toolbar">
                <div className="file-table-view__filters">
                  <Input
                    className="file-table-view__search"
                    prefix={<SearchOutlined />}
                    allowClear
                    aria-label={i18nText('frontstage', 'files.search')}
                    placeholder={i18nText('frontstage', 'files.search')}
                    value={search}
                    onChange={(event) => setSearch(event.target.value)}
                  />
                </div>
                <Space wrap>
                  <Button
                    type="primary"
                    icon={<UploadOutlined />}
                    aria-label={i18nText('frontstage', 'files.upload')}
                    disabled={!table || !csrfToken}
                    onClick={() => setUploadOpen(true)}
                  >
                    {i18nText('frontstage', 'files.upload_short')}
                  </Button>
                  <Button
                    icon={<ReloadOutlined />}
                    aria-label={i18nText('frontstage', 'files.refresh')}
                    disabled={!table}
                    loading={records.isFetching}
                    onClick={() => void records.refetch()}
                  >
                    {i18nText('frontstage', 'auto.refresh')}
                  </Button>
                  <DataTableColumnSettings
                    columns={columns}
                    configuration={configuration}
                  />
                  {toolbarExtra}
                </Space>
              </div>
              <div className="file-table-view__summary">
                <FileTextOutlined />
                <strong>
                  {table?.title ?? i18nText('frontstage', 'files.list')}
                </strong>
                <span>
                  ·{' '}
                  {i18nText('frontstage', 'files.count', {
                    count: records.data?.total ?? 0
                  })}
                </span>
              </div>
            </>
          }
        />
      </DataTableLayout>
      {shareTarget &&
      shareTarget.tableId === table?.id &&
      shareTarget.workspaceId === workspaceId &&
      csrfToken ? (
        <FileShareModal
          key={`${shareTarget.tableId}:${shareTarget.record.id}:${workspaceId}`}
          fileTableId={shareTarget.tableId}
          recordId={shareTarget.record.id}
          filename={shareTarget.record.filename}
          csrfToken={csrfToken}
          onClose={() => setShareTarget(undefined)}
        />
      ) : null}
      <Modal
        open={uploadOpen}
        title={i18nText('frontstage', 'files.upload')}
        getContainer={surface?.overlayHost.getPopupContainer}
        destroyOnHidden
        mask={{ closable: !uploading }}
        closable={!uploading}
        onCancel={() => {
          if (!uploading) {
            setUploadOpen(false);
            setFileList([]);
          }
        }}
        footer={
          <Button
            disabled={uploading}
            onClick={() => {
              setUploadOpen(false);
              setFileList([]);
            }}
          >
            {i18nText('frontstage', 'files.close')}
          </Button>
        }
      >
        <Upload.Dragger
          multiple
          fileList={fileList}
          customRequest={customRequest}
          onChange={({ fileList: next }) => setFileList(next)}
        >
          <p className={`${uploadPrefixCls}-drag-icon`}>
            <InboxOutlined />
          </p>
          <p className={`${uploadPrefixCls}-text`}>
            {i18nText('frontstage', 'files.upload_prompt')}
          </p>
          <p className={`${uploadPrefixCls}-hint`}>
            {i18nText('frontstage', 'files.upload_hint')}
          </p>
        </Upload.Dragger>
      </Modal>
    </div>
  );
}
