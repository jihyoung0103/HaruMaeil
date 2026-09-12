export interface DayItem {
  id: string;
  title: string;
  kind: 'event' | 'task';
  start?: Date;
  end?: Date;
  due?: Date;
  done?: boolean;
  source: string;
}

/** 로컬 기준 YYYY-MM-DD. toISOString()은 UTC로 밀려서 쓰면 안 됨. */
export const ymd = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;

/**
 * month는 1-12. 항상 6주(42칸) 고정 — 달마다 격자 높이가 튀지 않게.
 * Date 생성자가 음수/초과 일자를 알아서 이웃 달로 넘겨줌 (윤년 포함).
 */
export const monthCells = (year: number, month: number): Date[] => {
  const firstDow = new Date(year, month - 1, 1).getDay();
  return Array.from({ length: 42 }, (_, i) => new Date(year, month - 1, 1 - firstDow + i));
};

export const itemDate = (i: DayItem) => i.start ?? i.due;

/** 날짜별로 묶기. 칸 안에서는 task 먼저, 그다음 event 시간순. */
export function byDay(items: DayItem[]): Map<string, DayItem[]> {
  const m = new Map<string, DayItem[]>();
  for (const i of items) {
    const d = itemDate(i);
    if (!d) continue;
    const list = m.get(ymd(d));
    if (list) list.push(i);
    else m.set(ymd(d), [i]);
  }
  const rank = (i: DayItem) => (i.kind === 'task' ? 0 : 1);
  for (const list of m.values())
    list.sort((a, b) => rank(a) - rank(b) || (a.start?.getTime() ?? 0) - (b.start?.getTime() ?? 0));
  return m;
}

// ponytail: 여러 날 걸치는 event는 시작일 칸에만 뜸. 띠로 그리려면 셀 렌더를 손봐야 함.
// ponytail: 마감 지난 미완료 task도 원래 due 칸에 그대로. 오늘로 끌어올리려면 여기서 due를 보정.
