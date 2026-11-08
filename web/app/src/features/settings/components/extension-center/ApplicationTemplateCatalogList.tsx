import type { ApplicationTemplateCatalogItem } from '@1flowbase/api-client';
import { Button, Empty, Listy, Spin } from 'antd';
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { i18nText } from '../../../../shared/i18n/text';
import './application-template-catalog.css';

type CatalogRow =
  | { key: string; template: ApplicationTemplateCatalogItem }
  | { key: 'load-more'; template?: never };

export function ApplicationTemplateCatalogList({
  items,
  total,
  loading,
  failed,
  hasMore,
  canInstall,
  onLoadMore,
  onInstall
}: {
  items: ApplicationTemplateCatalogItem[];
  total: number;
  loading: boolean;
  failed: boolean;
  hasMore: boolean;
  canInstall: boolean;
  onLoadMore: () => Promise<unknown>;
  onInstall: (item: ApplicationTemplateCatalogItem) => void;
}) {
  const { t } = useTranslation('settingsSystemTemplates');
  const viewport = useRef<HTMLDivElement>(null);
  const [sentinel, setSentinel] = useState<HTMLDivElement | null>(null);
  const [height, setHeight] = useState(1);
  useLayoutEffect(() => {
    const element = viewport.current;
    if (!element) return;
    const measure = () => setHeight(Math.max(1, element.clientHeight));
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    measure();
    return () => observer.disconnect();
  }, []);
  const rows = useMemo<CatalogRow[]>(
    () => [
      ...items.map((template) => ({ key: template.catalog_id, template })),
      ...(hasMore ? [{ key: 'load-more' as const }] : [])
    ],
    [items, hasMore]
  );
  // The sentinel fills short viewports too; Listy's virtual window mounts it
  // again when scrolling reaches the end of the loaded rows.
  useEffect(() => {
    if (!hasMore || loading || failed) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) void onLoadMore();
      },
      { root: viewport.current, rootMargin: '200px' }
    );
    if (sentinel) observer.observe(sentinel);
    return () => observer.disconnect();
  }, [hasMore, loading, failed, onLoadMore, sentinel]);

  const headers = [
    t('template_name'),
    t('description'),
    t('version'),
    t('installed_version'),
    t('source'),
    t('action')
  ];
  return (
    <div className="application-template-catalog__list">
      <div
        role="table"
        aria-label={t('catalog_title')}
        aria-rowcount={total + 1}
        aria-colcount={6}
        aria-busy={loading}
        className="application-template-catalog__table"
      >
        <div role="row" className="application-template-catalog__header">
          {headers.map((title) => (
            <div role="columnheader" key={title}>
              {title}
            </div>
          ))}
        </div>
        <div ref={viewport} className="application-template-catalog__viewport">
          {rows.length ? (
            <Listy<CatalogRow>
              virtual
              height={height}
              items={rows}
              rowKey="key"
              styles={{
                root: { border: 0, borderRadius: 0 },
                item: { padding: 0, border: 0 }
              }}
              onScroll={(event) => {
                const { scrollTop, clientHeight, scrollHeight } =
                  event.currentTarget;
                if (!failed && scrollHeight - scrollTop - clientHeight < 200)
                  void onLoadMore();
              }}
              itemRender={(row, index) => {
                if (!row.template)
                  return (
                    <div
                      ref={setSentinel}
                      className="application-template-catalog__sentinel"
                    />
                  );
                const item = row.template;
                const values = [
                  item.name,
                  item.description,
                  item.release_version,
                  item.installed_release_version ?? t('not_installed'),
                  item.source === 'builtin'
                    ? t('source_builtin')
                    : t('source_official')
                ];
                return (
                  <div
                    role="row"
                    aria-rowindex={index + 2}
                    className="application-template-catalog__row"
                  >
                    {values.map((value, column) => (
                      <div
                        role="cell"
                        key={column}
                        className={`application-template-catalog__cell application-template-catalog__cell--${column}`}
                      >
                        <span className="application-template-catalog__label">
                          {headers[column]}
                        </span>
                        <span
                          className="application-template-catalog__value"
                          title={String(value)}
                        >
                          {value}
                        </span>
                      </div>
                    ))}
                    <div
                      role="cell"
                      className="application-template-catalog__action"
                    >
                      <Button
                        type="text"
                        disabled={!canInstall}
                        onClick={() => onInstall(item)}
                      >
                        {t('install')}
                      </Button>
                    </div>
                  </div>
                );
              }}
            />
          ) : !loading && !failed ? (
            <Empty description={t('catalog_empty')} />
          ) : null}
        </div>
      </div>
      <div className="application-template-catalog__status" role="status">
        {loading ? (
          <Spin size="small" />
        ) : !failed && items.length > 0 ? (
          <span>
            {i18nText(
              'sharedUi',
              hasMore ? 'auto.list_load_more' : 'auto.list_end'
            )}
          </span>
        ) : null}
      </div>
    </div>
  );
}
