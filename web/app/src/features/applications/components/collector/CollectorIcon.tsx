import { useState } from 'react';
import type { ConsoleApplicationCollector } from '@1flowbase/api-client';

export function CollectorIcon({
  collector
}: {
  collector: Pick<ConsoleApplicationCollector, 'display_name' | 'icon'>;
}) {
  const [failedIcon, setFailedIcon] = useState<string | null>(null);
  const { icon, display_name } = collector;

  return (
    <span className="application-collector__logo" aria-hidden="true">
      {icon && icon !== failedIcon ? (
        <img src={icon} alt="" onError={() => setFailedIcon(icon)} />
      ) : (
        display_name.slice(0, 1)
      )}
    </span>
  );
}
