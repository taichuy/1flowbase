import { useQuery } from '@tanstack/react-query';
import { Button, Select, Space, Spin, type SelectProps } from 'antd';
import { useState, type AriaAttributes } from 'react';
import { i18nText } from '../../../../../shared/i18n/text';

import { DirectoryReadError } from './DirectoryReadError';

type PageRequest = { page: number; page_size: number; keyword?: string };
type Page<T> = { items: T[]; page: number; page_size: number; total: number };

type DirectoryOption<T> = { value: string; label: string; item: T };

type Props<T> = AriaAttributes &
  Pick<SelectProps<string>, 'id' | 'value' | 'disabled' | 'className' | 'onBlur'> & {
    queryKey: readonly string[];
    fetchPage: (request: PageRequest) => Promise<Page<T>>;
    getId: (item: T) => string;
    getLabel: (item: T) => string;
    selected?: T;
    onPick: (item: T) => void;
    onChange?: (id: string) => void;
  };

/** The dropdown owns its search and page; the form owns the chosen record. */
export function DirectoryPagedSelect<T>({
  queryKey, fetchPage, getId, getLabel, selected, onPick, onChange, value, ...inputProps
}: Props<T>) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [page, setPage] = useState(1);
  const request = { page, page_size: 50, keyword: search.trim() || undefined };
  const query = useQuery({
    queryKey: [...queryKey, 'directory-picker', request],
    queryFn: () => fetchPage(request),
    enabled: open && !inputProps.disabled
  });
  const hasNext = query.data && page * query.data.page_size < query.data.total;
  const options = query.isError ? [] : query.data?.items.map((item) => ({
    value: getId(item), label: getLabel(item), item
  }));
  return (
    <Select<string, DirectoryOption<T>>
      {...inputProps}
      showSearch
      filterOption={false}
      value={value || undefined}
      labelRender={({ label, value: id }) =>
        selected && getId(selected) === id ? getLabel(selected) : label ?? id
      }
      loading={query.isFetching}
      searchValue={search}
      onSearch={(keyword) => {
        setSearch(keyword);
        setPage(1);
      }}
      onInputKeyDown={(event) => {
        if (!open) return;
        if (event.key === 'PageDown' && hasNext) {
          event.preventDefault();
          setPage(page + 1);
        } else if (event.key === 'PageUp' && page > 1) {
          event.preventDefault();
          setPage(page - 1);
        }
      }}
      onOpenChange={(visible) => {
        setOpen(visible);
        if (!visible) {
          setSearch('');
          setPage(1);
        }
      }}
      options={options}
      onChange={(id, option) => {
        if (option && !Array.isArray(option)) {
          onPick(option.item);
          onChange?.(id);
        }
      }}
      notFoundContent={query.isPending ? <Spin size="small" /> : query.isError ? (
        <DirectoryReadError error={query.error} retry={() => void query.refetch()} />
      ) : undefined}
      popupRender={(menu) => (
        <>
          {menu}
          {page > 1 || hasNext ? (
            <Space onMouseDown={(e) => e.preventDefault()}>
              <Button disabled={page === 1} onClick={() => setPage(page - 1)}>
                {i18nText('settings', 'auto.previous_page')}
              </Button>
              <span>{page}</span>
              <Button disabled={!hasNext} onClick={() => setPage(page + 1)}>
                {i18nText('settings', 'auto.next_page')}
              </Button>
            </Space>
          ) : null}
        </>
      )}
    />
  );
}
