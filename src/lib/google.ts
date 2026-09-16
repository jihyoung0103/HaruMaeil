import { invoke } from '@tauri-apps/api/core';
import { ymd, type DayItem } from './calendar';
import { replaceSource } from './db';

export interface GoogleStatus {
  hasClientId: boolean;
  connected: boolean;
}

interface RemoteEvent {
  id: string;
  title: string;
  start: string;
  end: string | null;
  allDay: boolean;
}

interface RemoteTask {
  id: string;
  title: string;
  /** YYYY-MM-DD */
  due: string;
  done: boolean;
}

export const googleStatus = () => invoke<GoogleStatus>('google_status');
export const googleConnect = () => invoke<void>('google_connect');
export const googleDisconnect = () => invoke<void>('google_disconnect');

/**
 * "2026-09-17"처럼 날짜만 있는 문자열을 new Date()에 그냥 넣으면 UTC 자정으로 읽혀
 * 시간대에 따라 하루 밀린다. 종일 일정은 로컬 자정으로 직접 만든다.
 */
function parseWhen(s: string): Date {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  return m ? new Date(+m[1], +m[2] - 1, +m[3]) : new Date(s);
}

/**
 * 구글 캘린더 일정과 할 일을 구간만큼 받아 SQLite의 google 항목을 갈아끼운다.
 * 할 일도 지금은 일정과 같은 모양으로 달력에 띄운다.
 */
export async function syncGoogle(from: Date, to: Date): Promise<{ events: number; tasks: number }> {
  const end = new Date(to.getFullYear(), to.getMonth(), to.getDate() + 1);

  const [events, tasks] = await Promise.all([
    invoke<RemoteEvent[]>('google_events', {
      timeMin: from.toISOString(),
      timeMax: end.toISOString()
    }),
    // 할 일 마감은 날짜만 의미가 있고 구글은 그걸 UTC 자정으로 저장한다.
    // 로컬 자정을 ISO로 바꿔 넘기면 UTC보다 느린 시간대에서 첫날이 빠지므로 날짜로 경계를 만든다
    invoke<RemoteTask[]>('google_tasks', {
      dueMin: `${ymd(from)}T00:00:00.000Z`,
      dueMax: `${ymd(end)}T00:00:00.000Z`
    })
  ]);

  const items: DayItem[] = [
    ...events.map(
      (r): DayItem => ({
        id: r.id,
        title: r.title,
        kind: 'event',
        start: parseWhen(r.start),
        end: r.end ? parseWhen(r.end) : undefined,
        source: 'google'
      })
    ),
    ...tasks.map(
      (r): DayItem => ({
        id: r.id,
        title: r.title,
        kind: 'task',
        due: parseWhen(r.due),
        done: r.done,
        source: 'google'
      })
    )
  ];

  await replaceSource('google', from, to, items);
  return { events: events.length, tasks: tasks.length };
}
