import { PlusOutlined } from '@ant-design/icons';
import { useMemo, useState } from 'react';

import { PageTreeIcon, pageTreeIconNames } from './registry';

const COLUMN_COUNT = 9;
const CELL_SIZE = 44;
const VIEWPORT_HEIGHT = 320;
const OVERSCAN_ROWS = 2;

type PageTreeIconPickerProps = {
  selectedIcon?: string;
  onSelect: (name: string) => void;
};

function PageTreeIconPicker({
  selectedIcon,
  onSelect
}: PageTreeIconPickerProps) {
  const [scrollTop, setScrollTop] = useState(0);
  const rowCount = Math.ceil(pageTreeIconNames.length / COLUMN_COUNT);
  const visibleRows = Math.ceil(VIEWPORT_HEIGHT / CELL_SIZE);
  const startRow = Math.max(
    0,
    Math.floor(scrollTop / CELL_SIZE) - OVERSCAN_ROWS
  );
  const endRow = Math.min(rowCount, startRow + visibleRows + OVERSCAN_ROWS * 2);
  const visibleIcons = useMemo(
    () =>
      pageTreeIconNames.slice(startRow * COLUMN_COUNT, endRow * COLUMN_COUNT),
    [endRow, startRow]
  );

  return (
    <div
      className="frontstage-page-tree-form__icon-viewport"
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
    >
      <div
        className="frontstage-page-tree-form__icon-spacer"
        style={{ height: rowCount * CELL_SIZE }}
      >
        <div
          className="frontstage-page-tree-form__icon-grid"
          style={{ transform: `translateY(${startRow * CELL_SIZE}px)` }}
        >
          {visibleIcons.map((iconName) => (
            <button
              key={iconName}
              aria-label={iconName}
              className={[
                'frontstage-page-tree-form__icon-button',
                selectedIcon === iconName
                  ? 'frontstage-page-tree-form__icon-button--selected'
                  : null
              ]
                .filter(Boolean)
                .join(' ')}
              type="button"
              onClick={() => onSelect(iconName)}
            >
              <PageTreeIcon name={iconName} fallback={<PlusOutlined />} />
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

export { PageTreeIconPicker };
