// node --experimental-strip-types src/lib/calendar.test.ts
import { monthCells, ymd, byDay, type DayItem } from './calendar.ts';

const ok = (cond: boolean, msg: string) => {
  if (!cond) throw new Error(msg);
};

// 2026-09: 1일이 화요일 → 앞에 일·월 두 칸이 8월
const sep = monthCells(2026, 9);
ok(sep.length === 42, '42칸 고정');
ok(sep[0].getDay() === 0, '첫 칸은 일요일');
ok(ymd(sep[0]) === '2026-08-30', '앞 달 빈칸');
ok(ymd(sep[2]) === '2026-09-01', '1일 위치');

// 윤년: 2024-02는 29일까지, 2025-02는 28일까지
ok(monthCells(2024, 2).some((d) => ymd(d) === '2024-02-29'), '2024는 윤년');
ok(!monthCells(2025, 2).some((d) => ymd(d) === '2025-02-29'), '2025는 평년');

// 연말/연초 넘어가기
ok(ymd(monthCells(2025, 12)[0]) === '2025-11-30', '12월 시작');
ok(ymd(monthCells(2026, 1)[0]) === '2025-12-28', '1월은 작년으로');

// 정렬: 같은 날이면 task 먼저, event는 시간순
const items: DayItem[] = [
  { id: 'b', title: '저녁', kind: 'event', start: new Date(2026, 8, 12, 19), source: 'l' },
  { id: 'a', title: '점심', kind: 'event', start: new Date(2026, 8, 12, 12), source: 'l' },
  { id: 't', title: '이체', kind: 'task', due: new Date(2026, 8, 12), source: 'l' }
];
ok(byDay(items).get('2026-09-12')?.map((i) => i.id).join() === 't,a,b', 'task 먼저, event 시간순');

console.log('ok');
