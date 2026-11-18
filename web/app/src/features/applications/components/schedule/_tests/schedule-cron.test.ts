import { describe, expect, it } from 'vitest';
import {
  DEFAULT_SCHEDULE_PATTERN,
  parseScheduleCron,
  serializeScheduleCron
} from '../schedule-cron';

describe('visual schedule cron encoding', () => {
  it.each([
    [{ frequency: 'minutes', interval: 5 }, '*/5 * * * *'],
    [{ frequency: 'minutes', interval: 1 }, '* * * * *'],
    [{ frequency: 'hours', interval: 2, minute: 15 }, '15 */2 * * *'],
    [{ frequency: 'hours', interval: 1, minute: 0 }, '0 * * * *'],
    [{ frequency: 'daily', hour: 23, minute: 59 }, '59 23 * * *'],
    [{ frequency: 'weekly', hour: 8, minute: 30, weekdays: [1, 3, 5] }, '30 8 * * 1,3,5'],
    [{ frequency: 'weekly', hour: 0, minute: 0, weekdays: [0] }, '0 0 * * 0'],
    [{ frequency: 'monthly', hour: 9, minute: 0, monthDay: 31 }, '0 9 31 * *']
  ] as const)('encodes %j as %s', (patch, expected) => {
    expect(serializeScheduleCron({
      ...DEFAULT_SCHEDULE_PATTERN,
      ...patch,
      weekdays: 'weekdays' in patch ? [...patch.weekdays] : DEFAULT_SCHEDULE_PATTERN.weekdays
    })).toBe(expected);
  });

  it('opens a saved weekday range as a weekly schedule', () => {
    expect(parseScheduleCron('0 9 * * 1-5')).toMatchObject({
      frequency: 'weekly', hour: 9, minute: 0, weekdays: [1, 2, 3, 4, 5]
    });
  });

  it('recognizes both Sunday aliases', () => {
    expect(parseScheduleCron('30 8 * * 7')).toMatchObject({ weekdays: [0] });
    expect(parseScheduleCron('30 8 * * 0')).toMatchObject({ weekdays: [0] });
  });

  it.each([
    '0 9 * 1,6 *',
    '0 9 1 * 1',
    '0,30 9 * * *',
    '0 9-17 * * *',
    '0 9 * * 1/2',
    '60 9 * * *',
    '0 24 * * *',
    '*/0 * * * *',
    ''
  ])('keeps unsupported or invalid expressions in custom mode: %s', (cron) => {
    expect(parseScheduleCron(cron)).toBeNull();
  });
});
