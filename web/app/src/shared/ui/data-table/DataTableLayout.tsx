import { Button } from 'antd';
import { useState } from 'react';
import type { FormEvent, ReactNode } from 'react';

import './data-table-layout.css';

export function DataTableLayout({
  children,
  className,
  filters
}: {
  children: ReactNode;
  className?: string;
  filters?: ReactNode;
}) {
  return (
    <section
      className={['data-table-layout', className].filter(Boolean).join(' ')}
    >
      {filters ? (
        <div className="data-table-layout__filter-region">{filters}</div>
      ) : null}
      <div className="data-table-layout__table-region">{children}</div>
    </section>
  );
}

export function DataTableFilterField({
  children,
  label
}: {
  children: ReactNode;
  label: ReactNode;
}) {
  return (
    <label className="data-table-filter-field">
      <span className="data-table-filter-field__label">{label}</span>
      <span className="data-table-filter-field__control">{children}</span>
    </label>
  );
}

export function DataTableFilterForm({
  ariaLabel,
  children,
  collapseLabel,
  expandLabel,
  expandedFields,
  resetLabel,
  submitLabel,
  onReset,
  onSubmit
}: {
  ariaLabel: string;
  children: ReactNode;
  collapseLabel: string;
  expandLabel: string;
  expandedFields?: ReactNode;
  resetLabel: string;
  submitLabel: string;
  onReset: () => void;
  onSubmit: () => void;
}) {
  const [expanded, setExpanded] = useState(false);

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    onSubmit();
  }

  return (
    <form
      aria-label={ariaLabel}
      className="data-table-filter-form"
      onSubmit={handleSubmit}
    >
      {children}
      {expanded ? expandedFields : null}
      <div className="data-table-filter-form__actions">
        {expandedFields ? (
          <Button
            aria-expanded={expanded}
            htmlType="button"
            onClick={() => setExpanded((current) => !current)}
          >
            {expanded ? collapseLabel : expandLabel}
          </Button>
        ) : null}
        <Button htmlType="button" onClick={onReset}>
          {resetLabel}
        </Button>
        <Button htmlType="submit" type="primary">
          {submitLabel}
        </Button>
      </div>
    </form>
  );
}
