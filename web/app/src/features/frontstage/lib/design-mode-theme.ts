/**
 * Shared "crystalline blue" design-mode palette.
 *
 * Single source of truth for the three configurable tiers (page title, page
 * tabs, blocks) so their hover/selected affordances read as one system and
 * match the page-tree sidebar's design-mode styling
 * (`frontstage-page-tree-sidebar.css`, the `--design` rules).
 *
 * The sidebar anchors the language:
 *   - design hover  → background rgba(59,130,246,0.045), text #2563eb
 *   - design select → border rgba(59,130,246,0.4), background rgba(59,130,246,0.12)
 *   - drop target   → dashed rgba(59,130,246,0.36), background rgba(59,130,246,0.07)
 */
export const FRONTSTAGE_DESIGN_BLUE = {
  /** Primary accent for text and icons in design mode. */
  primary: '#2563eb',
  /** Darker accent for hovered text/icons. */
  primaryStrong: '#1d4ed8',
  /** Idle outline so a configurable element reads as interactive. */
  borderIdle: 'rgba(59, 130, 246, 0.18)',
  /** Hover outline. */
  borderHover: 'rgba(59, 130, 246, 0.3)',
  /** Selected outline. */
  borderSelected: 'rgba(59, 130, 246, 0.4)',
  /** Hover background wash. */
  bgHover: 'rgba(59, 130, 246, 0.06)',
  /** Selected background wash. */
  bgSelected: 'rgba(59, 130, 246, 0.12)',
  /** Dashed outline for drop targets / empty affordances. */
  dashed: 'rgba(59, 130, 246, 0.36)',
  /** Dashed-target background wash. */
  bgDashed: 'rgba(59, 130, 246, 0.07)',
  /** 1px focus-halo used on hover framing. */
  halo: '0 0 0 1px rgba(59, 130, 246, 0.35)',
  /** Floating toolbar chrome. */
  toolbarBorder: 'rgba(59, 130, 246, 0.2)',
  toolbarShadow: '0 8px 24px rgba(37, 99, 235, 0.16)',
  /** Small pill label ("JS 区块" etc.) chrome. */
  labelBg: 'rgba(59, 130, 246, 0.1)',
  labelText: '#2563eb'
} as const;

export type FrontstageDesignBluePalette = typeof FRONTSTAGE_DESIGN_BLUE;
