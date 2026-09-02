import { describe, expect, it } from 'vitest';

import { planPageTreeIconPacks } from '../page-tree-icon-assets';

const inventory = [
  { baseName: 'AccountBook', name: 'AccountBookFilled', sourceBytes: 18 },
  { baseName: 'AccountBook', name: 'AccountBookOutlined', sourceBytes: 17 },
  { baseName: 'AccountBook', name: 'AccountBookTwoTone', sourceBytes: 25 },
  { baseName: 'Alert', name: 'AlertFilled', sourceBytes: 20 },
  { baseName: 'Alert', name: 'AlertOutlined', sourceBytes: 20 },
  { baseName: 'Bell', name: 'BellOutlined', sourceBytes: 20 }
];

describe('page tree icon asset planner', () => {
  it('MDP-007 produces deterministic packs independent of input order', () => {
    const first = planPageTreeIconPacks(inventory, 70);
    const second = planPageTreeIconPacks([...inventory].reverse(), 70);

    expect(second).toEqual(first);
    expect(first.map(({ id }) => id)).toEqual(
      first.map(({ id }) => expect.stringMatching(/^pack-[a-f0-9]{12}$/u))
    );
  });

  it('MDP-003 keeps icon theme families together while bounding packs', () => {
    const packs = planPageTreeIconPacks(inventory, 70);
    const accountBookPacks = packs.filter(({ icons }) =>
      icons.some(({ baseName }) => baseName === 'AccountBook')
    );

    expect(accountBookPacks).toHaveLength(1);
    expect(accountBookPacks[0]?.icons.map(({ name }) => name)).toEqual([
      'AccountBookFilled',
      'AccountBookOutlined',
      'AccountBookTwoTone'
    ]);
    expect(
      packs.every(
        ({ sourceBytes, icons }) =>
          sourceBytes <= 70 ||
          new Set(icons.map(({ baseName }) => baseName)).size === 1
      )
    ).toBe(true);
  });
});
