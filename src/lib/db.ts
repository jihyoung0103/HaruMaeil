import Database from '@tauri-apps/plugin-sql';
import { emit } from '@tauri-apps/api/event';
import type { Calendar, DayItem } from './calendar';

/** 쓰기가 일어나면 모든 창이 다시 읽게. 위젯도 이걸 듣는다 */
export const ITEMS_CHANGED = 'items-changed';

let conn: Promise<Database> | null = null;
const db = () => (conn ??= Database.load('sqlite:harumaeil.db'));

interface Row {
  id: string;
  calendar_id: string;
  title: string;
  kind: 'event' | 'task';
  at: number | null;
  end_at: number | null;
  done: number;
  all_day: number;
  color: string | null;
  calendar_color: string;
}

const toItem = (r: Row): DayItem => {
  const base = {
    id: r.id,
    title: r.title,
    calendarId: r.calendar_id,
    allDay: !!r.all_day,
    color: r.color ?? undefined,
    calendarColor: r.calendar_color
  };
  const at = r.at == null ? undefined : new Date(r.at);
  return r.kind === 'task'
    ? { ...base, kind: 'task', due: at, done: !!r.done }
    : { ...base, kind: 'event', start: at, end: r.end_at ? new Date(r.end_at) : undefined };
};

/** [from, to 다음날 자정) — to 날짜를 포함하는 구간의 ms 경계 */
const bounds = (from: Date, to: Date) => [
  from.getTime(),
  new Date(to.getFullYear(), to.getMonth(), to.getDate() + 1).getTime()
];

/** 구간에 걸치는 조건. 여러 날 일정이 구간 앞에서 시작해도 잡힌다. 날짜 없는 항목(at NULL)은 빠진다 */
// 컬럼에 테이블 이름을 안 붙인다 — calendars에는 at/end_at이 없어 JOIN에서도 모호하지 않고, DELETE에 별칭을 안 써도 된다
const OVERLAPS = 'at < $1 AND COALESCE(end_at, at) >= $2';

/** from~to(양끝 포함)에 걸치는 항목. 격자가 그리는 42칸을 그대로 넘기면 된다 */
export async function listRange(from: Date, to: Date): Promise<DayItem[]> {
  const [lo, hi] = bounds(from, to);
  const rows = await (
    await db()
  ).select<Row[]>(
    `SELECT i.*, c.color AS calendar_color FROM items i JOIN calendars c ON c.id = i.calendar_id
     WHERE ${OVERLAPS} ORDER BY at`,
    [hi, lo]
  );
  return rows.map(toItem);
}

const UPSERT_ITEM = `INSERT INTO items (id, calendar_id, title, kind, at, end_at, done, color, all_day)
     VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
     ON CONFLICT(id) DO UPDATE SET
       calendar_id=excluded.calendar_id, title=excluded.title, kind=excluded.kind,
       at=excluded.at, end_at=excluded.end_at, done=excluded.done, color=excluded.color,
       all_day=excluded.all_day`;

async function write(conn: Database, item: DayItem): Promise<void> {
  const at = (item.kind === 'task' ? item.due : item.start)?.getTime();
  // 스키마는 날짜 없는 항목을 허용하지만, 그걸 보여줄 곳(TODO P2-9 목록 패널)이 생기기 전까지는 막는다
  if (at === undefined) throw new Error('날짜 없는 항목은 저장할 수 없음');
  if (!item.title.trim()) throw new Error('제목이 비었음');
  await conn.execute(UPSERT_ITEM, [
    item.id,
    item.calendarId,
    item.title.trim(),
    item.kind,
    at,
    item.end?.getTime() ?? null,
    item.done ? 1 : 0,
    item.color ?? null,
    item.allDay ? 1 : 0
  ]);
}

export async function saveItem(item: DayItem): Promise<void> {
  await write(await db(), item);
  await emit(ITEMS_CHANGED);
}

/**
 * 원격에서 받은 캘린더들의 구간을 통째로 갈아끼운다. 캘린더(이름·색)를 먼저 넣어야
 * 항목의 외래 키가 맞는다. 증분 동기화(syncToken) 붙이기 전 방식.
 * ponytail: 항목당 execute 한 번. 수백 건 넘어가면 다중 행 INSERT로 묶을 것
 */
export async function replaceCalendars(
  calendars: Calendar[],
  from: Date,
  to: Date,
  items: DayItem[]
): Promise<void> {
  const [lo, hi] = bounds(from, to);
  const conn = await db();
  for (const c of calendars) {
    await conn.execute(
      `INSERT INTO calendars (id, name, color) VALUES ($1,$2,$3)
       ON CONFLICT(id) DO UPDATE SET name=excluded.name, color=excluded.color`,
      [c.id, c.name, c.color]
    );
    await conn.execute(`DELETE FROM items WHERE calendar_id = $3 AND ${OVERLAPS}`, [
      hi,
      lo,
      c.id
    ]);
  }
  for (const item of items) await write(conn, item);
  await emit(ITEMS_CHANGED);
}

export async function deleteItem(id: string): Promise<void> {
  await (await db()).execute('DELETE FROM items WHERE id = $1', [id]);
  await emit(ITEMS_CHANGED);
}
