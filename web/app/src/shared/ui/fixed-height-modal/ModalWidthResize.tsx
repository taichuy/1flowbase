import { useRef, type ReactNode } from 'react';

function clampModalWidth(width: number) {
  const maximum = Math.max(0, window.innerWidth - 32);
  return Math.min(maximum, Math.max(Math.min(560, maximum), width));
}

export function ModalWidthResize({
  children,
  onWidthChange
}: {
  children: ReactNode;
  onWidthChange: (width: number) => void;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; width: number; pointerId: number } | null>(
    null
  );
  return (
    <div ref={frame} className="fixed-height-modal__resize-frame">
      {children}
      {(['left', 'right'] as const).map((side) => (
        <div
          key={side}
          role="separator"
          aria-label={`Resize dialog from ${side}`}
          aria-orientation="vertical"
          tabIndex={0}
          className={`fixed-height-modal__resize-handle fixed-height-modal__resize-handle--${side}`}
          onPointerDown={(event) => {
            if (event.button !== 0) return;
            event.preventDefault();
            drag.current = {
              x: event.clientX,
              width: frame.current!.getBoundingClientRect().width,
              pointerId: event.pointerId
            };
            event.currentTarget.setPointerCapture(event.pointerId);
          }}
          onPointerMove={(event) => {
            const current = drag.current;
            if (!current || current.pointerId !== event.pointerId) return;
            const delta =
              (event.clientX - current.x) * (side === 'right' ? 2 : -2);
            onWidthChange(clampModalWidth(current.width + delta));
          }}
          onPointerUp={(event) => {
            if (drag.current?.pointerId !== event.pointerId) return;
            drag.current = null;
            event.currentTarget.releasePointerCapture(event.pointerId);
          }}
          onPointerCancel={() => {
            drag.current = null;
          }}
          onLostPointerCapture={() => {
            drag.current = null;
          }}
          onKeyDown={(event) => {
            if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
            event.preventDefault();
            const direction =
              (event.key === 'ArrowRight' ? 1 : -1) *
              (side === 'right' ? 1 : -1);
            onWidthChange(
              clampModalWidth(
                frame.current!.getBoundingClientRect().width + direction * 32
              )
            );
          }}
        />
      ))}
    </div>
  );
}
