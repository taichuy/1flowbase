import { describe, expect, it } from 'vitest';
import {
  FALLBACK_RETENTION_COST_BYTES,
  readHeapSample,
  readRetentionBudget,
  retainedMarkupCost,
  RetentionPressure,
  serializedCost
} from '../retention-memory';

describe('retention resource accounting', () => {
  it('validates optional Chromium memory instead of treating missing values as pressure', () => {
    for (const memory of [
      undefined,
      {},
      { usedJSHeapSize: NaN, jsHeapSizeLimit: 100 },
      { usedJSHeapSize: 101, jsHeapSizeLimit: 100 },
      { usedJSHeapSize: -1, jsHeapSizeLimit: 100 }
    ]) {
      expect(readHeapSample({ memory })).toBeNull();
    }
    expect(
      readHeapSample({
        get memory() {
          throw new Error('denied');
        }
      })
    ).toBeNull();
    expect(
      readHeapSample({ memory: { usedJSHeapSize: 70, jsHeapSizeLimit: 100 } })
    ).toEqual({ used: 70, limit: 100 });
  });
  it('uses a configurable accounting budget with safe capability fallback', () => {
    expect(readRetentionBudget(null, null)).toBe(FALLBACK_RETENTION_COST_BYTES);
    expect(readRetentionBudget(null, { used: 100, limit: 800 })).toBe(100);
    expect(
      readRetentionBudget({ getItem: () => '{"costBudgetBytes":0}' }, null)
    ).toBe(0);
    expect(
      readRetentionBudget(
        { getItem: () => '{"costBudgetBytes":1073741824}' },
        null
      )
    ).toBe(1073741824);
    for (const value of [
      'bad',
      '{"costBudgetBytes":-1}',
      '{"costBudgetBytes":"8"}'
    ]) {
      expect(readRetentionBudget({ getItem: () => value }, null)).toBe(
        FALLBACK_RETENTION_COST_BYTES
      );
    }
    expect(
      readRetentionBudget(
        {
          getItem() {
            throw new Error('disabled');
          }
        },
        null
      )
    ).toBe(FALLBACK_RETENTION_COST_BYTES);
  });
  it('reduces an inactive working set once per pressure excursion, never chasing delayed GC', () => {
    const pressure = new RetentionPressure();
    expect(pressure.budget({ used: 80, limit: 100 }, 1000, 200)).toBe(100);
    for (let i = 0; i < 10; i++)
      expect(pressure.budget({ used: 80, limit: 100 }, 1000, 100)).toBe(1000);
    expect(pressure.budget({ used: 50, limit: 100 }, 1000, 100)).toBe(1000);
    expect(pressure.budget({ used: 80, limit: 100 }, 1000, 100)).toBe(50);
    expect(pressure.budget(null, 1000, 100)).toBe(1000);
    expect(pressure.budget({ used: 95, limit: 100 }, 1000, 0)).toBe(0);
  });
  it('accounts actual serialized markup including nested shadow trees and never persists input', () => {
    const root = document.createElement('div');
    root.innerHTML = '<section></section>';
    const shadow = root.firstElementChild!.attachShadow({ mode: 'open' });
    shadow.innerHTML = '<input><article>中文</article>';
    const child = shadow
      .querySelector('article')!
      .attachShadow({ mode: 'open' });
    child.innerHTML = '<b>nested</b>';
    const expected = [root.innerHTML, shadow.innerHTML, child.innerHTML].reduce(
      (sum, markup) => sum + new TextEncoder().encode(markup).byteLength,
      0
    );
    expect(retainedMarkupCost(root)).toBe(expected);
    expect(serializedCost({ name: '中文' })).toBe(
      new TextEncoder().encode('{"name":"中文"}').byteLength
    );
    expect(retainedMarkupCost(null)).toBe(0);
  });
});
