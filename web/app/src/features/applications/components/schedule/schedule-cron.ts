export type ScheduleFrequency = 'minutes' | 'hours' | 'daily' | 'weekly' | 'monthly';

export interface SchedulePattern {
  frequency: ScheduleFrequency;
  interval: number;
  minute: number;
  hour: number;
  monthDay: number;
  weekdays: number[];
}

export const DEFAULT_SCHEDULE_PATTERN: SchedulePattern = {
  frequency: 'daily',
  interval: 1,
  minute: 0,
  hour: 9,
  monthDay: 1,
  weekdays: [1]
};

export function serializeScheduleCron(pattern: SchedulePattern): string {
  const { frequency, interval, minute, hour, monthDay, weekdays } = pattern;
  const step = interval === 1 ? '*' : `*/${interval}`;
  switch (frequency) {
    case 'minutes':
      return `${step} * * * *`;
    case 'hours':
      return `${minute} ${step} * * *`;
    case 'daily':
      return `${minute} ${hour} * * *`;
    case 'weekly':
      return `${minute} ${hour} * * ${weekdays.join(',')}`;
    case 'monthly':
      return `${minute} ${hour} ${monthDay} * *`;
  }
}

export const DEFAULT_SCHEDULE_CRON = serializeScheduleCron(DEFAULT_SCHEDULE_PATTERN);

function integer(value: string, min: number, max: number): number | null {
  if (!/^\d+$/.test(value)) return null;
  const result = Number(value);
  return result >= min && result <= max ? result : null;
}

function interval(value: string, max: number): number | null {
  if (value === '*') return 1;
  const match = /^\*\/(\d+)$/.exec(value);
  return match ? integer(match[1], 1, max) : null;
}

function parseWeekdays(value: string): number[] | null {
  const days = new Set<number>();
  for (const part of value.split(',')) {
    const range = /^(\d)-(\d)$/.exec(part);
    const start = integer(range ? range[1] : part, 0, 7);
    const end = range ? integer(range[2], 0, 7) : start;
    if (start === null || end === null || start > end) return null;
    for (let day = start; day <= end; day += 1) days.add(day % 7);
  }
  return [...days].sort((a, b) => (a || 7) - (b || 7));
}

// Recognize only schedules that the visual controls can represent exactly.
// All other expressions remain editable in custom mode without rewriting them.
export function parseScheduleCron(value: string): SchedulePattern | null {
  const fields = value.trim().split(/\s+/);
  if (fields.length !== 5) return null;
  const [minuteField, hourField, dayField, monthField, weekdayField] = fields;
  if (monthField !== '*') return null;
  const pattern = { ...DEFAULT_SCHEDULE_PATTERN };
  const minuteInterval = interval(minuteField, 59);
  if (
    minuteInterval !== null && hourField === '*' &&
    dayField === '*' && weekdayField === '*'
  ) {
    return { ...pattern, frequency: 'minutes', interval: minuteInterval };
  }
  const minute = integer(minuteField, 0, 59);
  if (minute === null) return null;
  const hourInterval = interval(hourField, 23);
  if (hourInterval !== null && dayField === '*' && weekdayField === '*') {
    return { ...pattern, frequency: 'hours', interval: hourInterval, minute };
  }
  const hour = integer(hourField, 0, 23);
  if (hour === null) return null;
  if (dayField === '*' && weekdayField === '*') {
    return { ...pattern, frequency: 'daily', hour, minute };
  }
  if (dayField === '*') {
    const weekdays = parseWeekdays(weekdayField);
    return weekdays ? { ...pattern, frequency: 'weekly', hour, minute, weekdays } : null;
  }
  const monthDay = integer(dayField, 1, 31);
  if (monthDay !== null && weekdayField === '*') {
    return { ...pattern, frequency: 'monthly', hour, minute, monthDay };
  }
  return null;
}
