import Database from '@tauri-apps/plugin-sql';
import { emit } from '@tauri-apps/api/event';
import type { DayItem } from './calendar';

/** 쓰기가 일어나면 모든 창이 다시 읽게. 위젯도 이걸 듣는다 */
export const ITEMS_CHANGED = 'items-changed';

let conn: Promise<Database> | null = null;
const db = () => (conn ??= Database.load('sqlite:harumaeil.db'));

interface Row {
  id: string;
  title: string;
  kind: 'event' | 'task';
  at: number;
  end_at: number | null;
  done: number;
  source: string;
}

const toItem = (r: Row): DayItem =>
  r.kind === 'task'
    ? { id: r.id, title: r.title, kind: 'task', due: new Date(r.at), done: !!r.done, source: r.source }
    : {
        id: r.id,
        title: r.title,
        kind: 'event',
        start: new Date(r.at),
        end: r.end_at ? new Date(r.end_at) : undefined,
        source: r.source
      };

/** from~to(양끝 포함)에 걸치는 항목. 격자가 그리는 42칸을 그대로 넘기면 된다 */
export async function listRange(from: Date, to: Date): Promise<DayItem[]> {
  const lo = from.getTime();
  const hi = new Date(to.getFullYear(), to.getMonth(), to.getDate() + 1).getTime();
  const rows = await (
    await db()
  ).select<Row[]>('SELECT * FROM items WHERE at < $1 AND COALESCE(end_at, at) >= $2 ORDER BY at', [
    hi,
    lo
  ]);
  return rows.map(toItem);
}

const UPSERT = `INSERT INTO items (id, title, kind, at, end_at, done, source) VALUES ($1,$2,$3,$4,$5,$6,$7)
     ON CONFLICT(id) DO UPDATE SET
       title=excluded.title, kind=excluded.kind, at=excluded.at,
       end_at=excluded.end_at, done=excluded.done, source=excluded.source`;

async function write(conn: Database, item: DayItem): Promise<void> {
  const at = (item.kind === 'task' ? item.due : item.start)?.getTime();
  if (at === undefined) throw new Error('날짜 없는 항목은 저장할 수 없음');
  if (!item.title.trim()) throw new Error('제목이 비었음');
  await conn.execute(UPSERT, [
    item.id,
    item.title.trim(),
    item.kind,
    at,
    item.end?.getTime() ?? null,
    item.done ? 1 : 0,
    item.source
  ]);
}

export async function saveItem(item: DayItem): Promise<void> {
  await write(await db(), item);
  await emit(ITEMS_CHANGED);
}

/**
 * 구간 안의 해당 source 항목을 통째로 갈아끼운다. 증분 동기화(syncToken) 붙이기 전 방식.
 * ponytail: 항목당 execute 한 번. 수백 건 넘어가면 다중 행 INSERT로 묶을 것
 */
export async function replaceSource(
  source: string,
  from: Date,
  to: Date,
  items: DayItem[]
): Promise<void> {
  const lo = from.getTime();
  const hi = new Date(to.getFullYear(), to.getMonth(), to.getDate() + 1).getTime();
  const conn = await db();
  await conn.execute('DELETE FROM items WHERE source = $1 AND at >= $2 AND at < $3', [
    source,
    lo,
    hi
  ]);
  for (const item of items) await write(conn, item);
  await emit(ITEMS_CHANGED);
}

export async function deleteItem(id: string): Promise<void> {
  await (await db()).execute('DELETE FROM items WHERE id = $1', [id]);
  await emit(ITEMS_CHANGED);
}
