import { invoke } from '@tauri-apps/api/core';
import type { DayItem } from './calendar';
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

/** 구글 캘린더에서 구간을 받아 SQLite의 google 항목을 갈아끼운다. 가져온 건수를 돌려준다 */
export async function syncGoogle(from: Date, to: Date): Promise<number> {
  const rows = await invoke<RemoteEvent[]>('google_events', {
    timeMin: from.toISOString(),
    timeMax: new Date(to.getFullYear(), to.getMonth(), to.getDate() + 1).toISOString()
  });

  const items: DayItem[] = rows.map((r) => ({
    id: r.id,
    title: r.title,
    kind: 'event',
    start: parseWhen(r.start),
    end: r.end ? parseWhen(r.end) : undefined,
    source: 'google'
  }));

  await replaceSource('google', from, to, items);
  return items.length;
}
