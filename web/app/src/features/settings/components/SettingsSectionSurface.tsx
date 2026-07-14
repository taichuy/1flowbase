import type { ReactNode } from 'react';

import './settings-section-surface.css';

export function SettingsSectionSurface({
  children,
  toolbar,
  status,
  heightMode = 'auto'
}: {
  children: ReactNode;
  toolbar?: ReactNode;
  status?: ReactNode;
  heightMode?: 'auto' | 'fill';
}) {
  return (
    <section
      className={[
        'settings-section-surface',
        heightMode === 'fill' ? 'settings-section-surface--fill' : null
      ]
        .filter(Boolean)
        .join(' ')}
      data-testid="settings-section-surface"
    >
      {toolbar ? (
        <div className="settings-section-surface__toolbar">{toolbar}</div>
      ) : null}

      {status ? (
        <div className="settings-section-surface__status">{status}</div>
      ) : null}

      <div className="settings-section-surface__body">{children}</div>
    </section>
  );
}
