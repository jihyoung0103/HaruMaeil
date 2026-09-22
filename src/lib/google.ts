import { invoke } from '@tauri-apps/api/core';
import { ymd, parseWhen, DEFAULT_COLOR, type Calendar, type DayItem } from './calendar';
import { replaceCalendars } from './db';

export interface GoogleStatus {
  hasClientId: boolean;
  connected: boolean;
}

interface RemoteCalendar {
  id: string;
  name: string;
  /** 할 일 목록은 구글이 색을 안 준다 */
  color: string | null;
}

interface RemoteEvent {
  id: string;
  title: string;
  start: string;
  end: string | null;
  allDay: boolean;
  /** 일정에 따로 고른 색 */
  color: string | null;
}

interface RemoteTask {
  id: string;
  list: string;
  title: string;
  /** YYYY-MM-DD */
  due: string;
  done: boolean;
}

export const googleStatus = () => invoke<GoogleStatus>('google_status');
export const googleConnect = () => invoke<void>('google_connect');
export const googleDisconnect = () => invoke<void>('google_disconnect');

/** 구글이 색을 안 주는 할 일 목록용. id로 골라서 목록마다 색이 항상 같게 */
const TASK_COLORS = ['#8e24aa', '#e67c73', '#f6bf26', '#039be5', '#f4511e', '#7986cb', '#616161'];
const colorFor = (id: string) =>
  TASK_COLORS[[...id].reduce((h, ch) => (h * 31 + ch.charCodeAt(0)) >>> 0, 0) % TASK_COLORS.length];

/** 구글 캘린더 일정과 할 일을 구간만큼 받아 SQLite의 해당 캘린더들을 갈아끼운다 */
export async function syncGoogle(from: Date, to: Date): Promise<{ events: number; tasks: number }> {
  const end = new Date(to.getFullYear(), to.getMonth(), to.getDate() + 1);

  const [cal, tasks] = await Promise.all([
    invoke<{ calendar: RemoteCalendar; events: RemoteEvent[] }>('google_events', {
      timeMin: from.toISOString(),
      timeMax: end.toISOString()
    }),
    // 할 일 마감은 날짜만 의미가 있고 구글은 그걸 UTC 자정으로 저장한다.
    // 로컬 자정을 ISO로 바꿔 넘기면 UTC보다 느린 시간대에서 첫날이 빠지므로 날짜로 경계를 만든다
    invoke<{ lists: RemoteCalendar[]; tasks: RemoteTask[] }>('google_tasks', {
      dueMin: `${ymd(from)}T00:00:00.000Z`,
      dueMax: `${ymd(end)}T00:00:00.000Z`
    })
  ]);

  const calendars: Calendar[] = [
    { ...cal.calendar, color: cal.calendar.color ?? DEFAULT_COLOR },
    ...tasks.lists.map((l) => ({ ...l, color: l.color ?? colorFor(l.id) }))
  ];

  const items: DayItem[] = [
    ...cal.events.map(
      (r): DayItem => ({
        id: r.id,
        title: r.title,
        kind: 'event',
        start: parseWhen(r.start),
        end: r.end ? parseWhen(r.end) : undefined,
        // start.dateTime 없이 start.date만 온 일정
        allDay: r.allDay,
        calendarId: cal.calendar.id,
        color: r.color ?? undefined
      })
    ),
    ...tasks.tasks.map(
      (r): DayItem => ({
        id: r.id,
        title: r.title,
        kind: 'task',
        due: parseWhen(r.due),
        done: r.done,
        // 구글 할 일 due는 00:00:00Z로 와도 날짜만 유효하다 — 시각 있음으로 보지 않는다
        allDay: true,
        calendarId: r.list
      })
    )
  ];

  await replaceCalendars(calendars, from, to, items);
  return { events: cal.events.length, tasks: tasks.tasks.length };
}
