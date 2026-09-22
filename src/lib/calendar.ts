/** 항목이 속한 캘린더 id. 구글은 'gcal:<캘린더 id>', 'gtask:<목록 id>' */
export const LOCAL_CALENDAR = 'local';
export const HOLIDAY_CALENDAR = 'holiday:kr';
export const DEFAULT_COLOR = '#4a86e8';

export interface DayItem {
  id: string;
  title: string;
  kind: 'event' | 'task';
  start?: Date;
  end?: Date;
  due?: Date;
  done?: boolean;
  /**
   * 시각 없이 날짜만 있는 항목. 구글 일정은 start.dateTime이 없으면 종일, 구글 할 일은 due가
   * 00:00:00Z로 와도 날짜만 유효하므로 항상 종일. 자정 시작 여부로 어림하지 말 것
   */
  allDay: boolean;
  calendarId: string;
  /** 이 항목에만 지정된 색 (구글에서 일정마다 고른 색). 없으면 calendarColor를 쓴다 */
  color?: string;
  /**
   * 조회할 때 calendars 테이블에서 붙는다. color와 따로 두는 이유: 읽은 항목을 다시 저장할 때
   * (로컬 할 일 체크 등) 캘린더 색이 항목 자기 색으로 박혀버리지 않게
   */
  calendarColor?: string;
}

export interface Calendar {
  id: string;
  name: string;
  color: string;
}

/** 0 = 일요일 시작, 1 = 월요일 시작 */
export type WeekStart = 0 | 1;

/** 로컬 기준 YYYY-MM-DD. toISOString()은 UTC로 밀려서 쓰면 안 됨. */
export const ymd = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;

/**
 * "2026-09-17"처럼 날짜만 있는 문자열을 new Date()에 그냥 넣으면 UTC 자정으로 읽혀
 * 시간대에 따라 하루 밀린다. 날짜만 있으면 로컬 자정으로 직접 만든다.
 */
export function parseWhen(s: string): Date {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  return m ? new Date(+m[1], +m[2] - 1, +m[3]) : new Date(s);
}

/**
 * month는 1-12. 항상 6주(42칸) 고정 — 달마다 격자 높이가 튀지 않게.
 * Date 생성자가 음수/초과 일자를 알아서 이웃 달로 넘겨줌 (윤년 포함).
 */
export const monthCells = (year: number, month: number, weekStart: WeekStart = 0): Date[] => {
  const lead = (new Date(year, month - 1, 1).getDay() - weekStart + 7) % 7;
  return Array.from({ length: 42 }, (_, i) => new Date(year, month - 1, 1 - lead + i));
};

export const itemDate = (i: DayItem) => i.start ?? i.due;

/** 14:00 */
export const hhmm = (d: Date) => `${d.getHours()}:${String(d.getMinutes()).padStart(2, '0')}`;

const dayStart = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
// 반올림: 서머타임이 있는 곳에선 하루가 23·25시간일 수 있다
const dayDiff = (a: Date, b: Date) => Math.round((dayStart(b).getTime() - dayStart(a).getTime()) / 86_400_000);

/**
 * 항목이 차지하는 첫날과 마지막 날(둘 다 자정). 끝 시각은 그 순간을 포함하지 않는다고 본다:
 * 종일 일정의 끝은 다음날 자정으로 오고, 22시~24시 일정이 다음날로 번지면 안 된다.
 */
export function itemDays(i: DayItem): [Date, Date] | null {
  const s = itemDate(i);
  if (!s) return null;
  const first = dayStart(s);
  if (i.kind !== 'event' || !i.end || i.end <= s) return [first, first];
  const last = dayStart(new Date(i.end.getTime() - 1));
  return [first, last];
}

export const coversDay = (i: DayItem, day: Date) => {
  const r = itemDays(i);
  return !!r && r[0] <= dayStart(day) && dayStart(day) <= r[1];
};

export interface Bar {
  item: DayItem;
  /** 0-6, 주 안에서 시작 칸 */
  col: number;
  span: number;
  /** 위에서부터 몇 번째 줄 */
  lane: number;
  /** 지난주에서 이어져 옴 / 다음주로 이어짐 — 막대 끝을 평평하게 그린다 */
  startsBefore: boolean;
  endsAfter: boolean;
}

/**
 * 한 주(7칸)에 걸치는 항목을 막대로 배치한다. 한 줄에 겹치지 않게 넣되,
 * 먼저 시작하는 것 → 긴 것 → 할 일 → 이른 시각 순으로 가장 위 빈 줄을 차지한다.
 */
export function layoutWeek(days: Date[], items: DayItem[]): Bar[] {
  const weekFirst = dayStart(days[0]);
  const weekLast = dayStart(days[6]);

  const segs: Omit<Bar, 'lane'>[] = [];
  for (const item of items) {
    const r = itemDays(item);
    if (!r || r[1] < weekFirst || r[0] > weekLast) continue;
    const col = Math.max(0, dayDiff(weekFirst, r[0]));
    const endCol = Math.min(6, dayDiff(weekFirst, r[1]));
    segs.push({
      item,
      col,
      span: endCol - col + 1,
      startsBefore: r[0] < weekFirst,
      endsAfter: r[1] > weekLast
    });
  }

  const rank = (i: DayItem) => (i.kind === 'task' ? 0 : 1);
  segs.sort(
    (a, b) =>
      a.col - b.col ||
      b.span - a.span ||
      rank(a.item) - rank(b.item) ||
      (a.item.start?.getTime() ?? 0) - (b.item.start?.getTime() ?? 0)
  );

  const used: boolean[][] = []; // used[줄][칸]
  return segs.map((s) => {
    let lane = 0;
    while (used[lane]?.slice(s.col, s.col + s.span).some(Boolean)) lane++;
    used[lane] ??= Array(7).fill(false);
    for (let c = s.col; c < s.col + s.span; c++) used[lane][c] = true;
    return { ...s, lane };
  });
}

/** lanes 줄 안에 못 들어가 안 보이는 막대가 요일 칸마다 몇 개인지 */
export function hiddenPerDay(bars: Bar[], lanes: number): number[] {
  const n = Array(7).fill(0);
  for (const b of bars) if (b.lane >= lanes) for (let c = b.col; c < b.col + b.span; c++) n[c]++;
  return n;
}

/** 사이드 패널이 위에서 아래로 그리는 묶음 */
export interface DayGroups {
  /** 공휴일·기념일 */
  holidays: DayItem[];
  /** 종일 일정 */
  allDay: DayItem[];
  /** 그날 할 일 전부 — 시각 유무와 상관없이 */
  tasks: DayItem[];
  /** 시각 있는 일정 + 시각 있는 할 일, 시작 시각 순 */
  timed: DayItem[];
}

/**
 * 하루치 항목을 묶음별 배열에 나눠 담는다. 한 배열을 정렬로 섞지 않는다 — 정렬하는 건 timed뿐.
 * 시각 있는 할 일은 tasks와 timed 양쪽에 **같은 객체로** 들어간다(의도적 중복). 복사하지 않으므로
 * 한쪽에서 완료 처리하면 양쪽이 같이 바뀐다.
 * 종일 일정의 끝은 다음날 자정이라 coversDay(끝을 포함하지 않음)로 걸러야 하루 밀리지 않는다.
 */
export function dayGroups(items: DayItem[], day: Date): DayGroups {
  const g: DayGroups = { holidays: [], allDay: [], tasks: [], timed: [] };
  for (const i of items) {
    if (!coversDay(i, day)) continue;
    if (i.calendarId === HOLIDAY_CALENDAR) g.holidays.push(i);
    else if (i.kind === 'task') {
      g.tasks.push(i);
      if (!i.allDay) g.timed.push(i);
    } else if (i.allDay) g.allDay.push(i);
    else g.timed.push(i);
  }
  g.timed.sort((a, b) => itemDate(a)!.getTime() - itemDate(b)!.getTime());
  return g;
}

// ponytail: 마감 지난 미완료 할 일도 원래 마감일 칸에 그대로. 규칙은 TODO P2-9와 같이 결정.
