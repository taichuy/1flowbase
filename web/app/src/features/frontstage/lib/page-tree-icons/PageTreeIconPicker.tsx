import { Input } from 'antd';
import { useMemo, useRef, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { pageTreeIconNames } from './metadata';
import { PageTreeIconPreview } from './preview';
import { IconSearchIndex } from './search-index';
import { iconWindow } from './window';

const COLUMN_COUNT = 9;
const CELL_SIZE = 44;
const VIEWPORT_HEIGHT = 320;
const OVERSCAN_ROWS = 2;
const iconSearchIndex = new IconSearchIndex(pageTreeIconNames);

type PageTreeIconPickerProps = {
  selectedIcon?: string;
  onSelect: (name: string) => void;
};

function PageTreeIconPicker({
  selectedIcon,
  onSelect
}: PageTreeIconPickerProps) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState('');
  const [scrollTop, setScrollTop] = useState(0);
  const matchingIcons = useMemo(() => iconSearchIndex.search(query), [query]);
  const rowCount = Math.ceil(matchingIcons.length / COLUMN_COUNT);
  const { startRow, startIndex, endIndex } = iconWindow({
    itemCount: matchingIcons.length,
    scrollTop,
    columnCount: COLUMN_COUNT,
    cellSize: CELL_SIZE,
    viewportHeight: VIEWPORT_HEIGHT,
    overscanRows: OVERSCAN_ROWS
  });
  const visibleIcons = useMemo(
    () => matchingIcons.slice(startIndex, endIndex),
    [endIndex, matchingIcons, startIndex]
  );

  return (
    <div className="frontstage-page-tree-form__icon-picker">
      <Input
        allowClear
        aria-label={i18nText('frontstage', 'auto.search_icons')}
        placeholder={i18nText('frontstage', 'auto.search_icons')}
        type="search"
        value={query}
        onChange={(event) => {
          setQuery(event.target.value);
          setScrollTop(0);
          if (viewportRef.current) viewportRef.current.scrollTop = 0;
        }}
      />
      <div
        ref={viewportRef}
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
                <PageTreeIconPreview name={iconName} />
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

export { PageTreeIconPicker };
