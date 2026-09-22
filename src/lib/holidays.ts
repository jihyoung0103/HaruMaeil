import { invoke } from '@tauri-apps/api/core';
import { HOLIDAY_CALENDAR, parseWhen, type Calendar, type DayItem } from './calendar';
import { replaceCalendars } from './db';

const CALENDAR: Calendar = { id: HOLIDAY_CALENDAR, name: '공휴일', color: '#0b8043' };

/** 이번 실행에서 이미 받은 해. 임시공휴일이 나중에 지정돼도 앱을 다시 켜면 반영된다 */
const fetched = new Set<number>();

/**
 * 격자에 보이는 해들의 공휴일을 받아 DB에 넣는다. 실행당 해마다 한 번만 묻고,
 * 실패하면 이전에 받아둔 것이 그대로 남는다.
 */
export async function ensureHolidays(from: Date, to: Date): Promise<void> {
  for (const year of new Set([from.getFullYear(), to.getFullYear()])) {
    if (fetched.has(year)) continue;
    fetched.add(year); // 실패해도 같은 실행에서 계속 다시 묻지 않게 먼저 표시

    const rows = await invoke<{ date: string; name: string }[]>('holidays', { year });
    const items: DayItem[] = rows.map((r) => ({
      // 같은 날 공휴일이 둘일 수 있다 (예: 어린이날과 부처님오신날이 겹친 2025-05-05)
      id: `${HOLIDAY_CALENDAR}:${r.date}:${r.name}`,
      title: r.name,
      kind: 'event',
      start: parseWhen(r.date),
      allDay: true,
      calendarId: HOLIDAY_CALENDAR
    }));
    await replaceCalendars([CALENDAR], new Date(year, 0, 1), new Date(year, 11, 31), items);
  }
}
