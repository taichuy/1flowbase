import { describe, expect, it } from 'vitest';

import { IconSearchIndex, iconSearchTerms } from '../search-index';

describe('IconSearchIndex', () => {
  const index = new IconSearchIndex([
    'AccountBookOutlined',
    'SmileFilled',
    'SmileOutlined',
    'SmileTwoTone',
    'UserSwitchOutlined'
  ]);

  it('MDP-003 uses unique trigrams for stable postings', () => {
    expect(iconSearchTerms('Outlined')).toEqual([
      'out',
      'utl',
      'tli',
      'lin',
      'ine',
      'ned'
    ]);
  });

  it('MDP-003 intersects trigram postings and preserves catalog order', () => {
    expect(index.search('smile')).toEqual([
      'SmileFilled',
      'SmileOutlined',
      'SmileTwoTone'
    ]);
    expect(index.search('switch')).toEqual(['UserSwitchOutlined']);
  });

  it('MDP-003 supports empty and short queries without false positives', () => {
    expect(index.search('')).toHaveLength(5);
    expect(index.search('tw')).toEqual(['SmileTwoTone']);
    expect(index.search('missing')).toEqual([]);
  });
});
