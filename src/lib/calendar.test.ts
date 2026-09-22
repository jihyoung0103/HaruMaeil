// node --experimental-strip-types src/lib/calendar.test.ts
import {
  monthCells,
  ymd,
  parseWhen,
  itemDays,
  coversDay,
  layoutWeek,
  hiddenPerDay,
  dayGroups,
  HOLIDAY_CALENDAR,
  type DayItem
} from './calendar.ts';

const ok = (cond: boolean, msg: string) => {
  if (!cond) throw new Error(msg);
};
const d = (y: number, m: number, day: number, h = 0, min = 0) => new Date(y, m - 1, day, h, min);
const ev = (id: string, start: Date, end?: Date, allDay = false): DayItem => ({
  id,
  title: id,
  kind: 'event',
  start,
  end,
  allDay,
  calendarId: 'c'
});
const task = (id: string, due: Date, allDay: boolean): DayItem => ({ id, title: id, kind: 'task', due, allDay, calendarId: 't' });

// ---- 격자 ----
// 2026-09: 1일이 화요일 → 일요일 시작이면 앞에 일·월 두 칸이 8월
const sep = monthCells(2026, 9);
ok(sep.length === 42, '42칸 고정');
ok(sep[0].getDay() === 0, '첫 칸은 일요일');
ok(ymd(sep[0]) === '2026-08-30', '앞 달 빈칸');
ok(ymd(sep[2]) === '2026-09-01', '1일 위치');

// 월요일 시작
const sepMon = monthCells(2026, 9, 1);
ok(sepMon[0].getDay() === 1, '월요일 시작이면 첫 칸은 월요일');
ok(ymd(sepMon[0]) === '2026-08-31', '월요일 시작 앞 달 빈칸 1칸');
// 2026-11-01은 일요일 → 월요일 시작이면 앞에 6칸
ok(ymd(monthCells(2026, 11, 1)[0]) === '2026-10-26', '1일이 일요일이면 월요일 시작에서 6칸 밀림');
ok(ymd(monthCells(2026, 11, 0)[0]) === '2026-11-01', '1일이 일요일이면 일요일 시작은 빈칸 없음');

// 윤년, 연말/연초
ok(monthCells(2024, 2).some((x) => ymd(x) === '2024-02-29'), '2024는 윤년');
ok(!monthCells(2025, 2).some((x) => ymd(x) === '2025-02-29'), '2025는 평년');
ok(ymd(monthCells(2025, 12)[0]) === '2025-11-30', '12월 시작');
ok(ymd(monthCells(2026, 1)[0]) === '2025-12-28', '1월은 작년으로');

// ---- 날짜 문자열 ----
ok(ymd(parseWhen('2026-09-25')) === '2026-09-25', '날짜만 있는 문자열은 로컬 날짜 그대로');
ok(parseWhen('2026-09-25').getHours() === 0, '로컬 자정');

// ---- 항목이 차지하는 날 ----
const days = (i: DayItem) => itemDays(i)!.map(ymd).join('~');
ok(days(ev('a', d(2026, 9, 17, 11))) === '2026-09-17~2026-09-17', '끝 없는 일정은 하루');
// 구글 종일 일정: 끝이 다음날 자정(배타적)
ok(days(ev('b', d(2026, 9, 30), d(2026, 10, 3))) === '2026-09-30~2026-10-02', '종일 3일짜리');
ok(days(ev('c', d(2026, 9, 17, 22), d(2026, 9, 18, 0))) === '2026-09-17~2026-09-17', '자정에 끝나면 다음날로 안 번짐');
ok(days(ev('e', d(2026, 9, 17, 23), d(2026, 9, 18, 1))) === '2026-09-17~2026-09-18', '밤샘 일정은 이틀');
ok(coversDay(ev('b', d(2026, 9, 30), d(2026, 10, 3)), d(2026, 10, 1)), '가운데 날도 포함');

// ---- 주 단위 막대 배치 ----
// 월요일 시작 주: 2026-09-28(월) ~ 10-04(일)
const week = monthCells(2026, 9, 1).slice(28, 35);
ok(ymd(week[0]) === '2026-09-28' && ymd(week[6]) === '2026-10-04', '테스트용 주 범위');

const bars = layoutWeek(week, [
  ev('capstone', d(2026, 9, 30), d(2026, 10, 3)), // 수~금, 3칸
  ev('lunch', d(2026, 10, 1, 12)), // 목, 캡스톤과 겹침 → 아래 줄
  ev('trip', d(2026, 9, 25), d(2026, 9, 30)), // 지난주부터 화요일까지
  ev('mon', d(2026, 9, 28, 9)), // 월, trip과 겹침
  ev('sun', d(2026, 10, 4, 10), d(2026, 10, 6)), // 일요일에서 다음주로
  ev('other', d(2026, 11, 1)) // 이 주 밖
]);
const by = (id: string) => bars.find((b) => b.item.id === id)!;

ok(bars.length === 5, '주 밖의 항목은 빠짐');
ok(by('capstone').col === 2 && by('capstone').span === 3, '캡스톤: 수요일부터 3칸');
ok(by('trip').col === 0 && by('trip').span === 2 && by('trip').startsBefore, '지난주에서 이어진 막대');
ok(by('sun').col === 6 && by('sun').span === 1 && by('sun').endsAfter, '다음주로 이어지는 막대');
ok(by('trip').lane === 0 && by('mon').lane === 1, '월요일에 겹치면 긴 막대가 위');
ok(by('capstone').lane === 0, '앞 칸이 빈 줄이면 위 줄 재사용');
ok(by('lunch').lane === 1, '캡스톤과 겹치는 목요일 점심은 둘째 줄');

const hidden = hiddenPerDay(bars, 1);
ok(hidden.join() === '1,0,0,1,0,0,0', '한 줄만 보일 때 월·목에 하나씩 숨음');

// ---- 사이드 패널 묶음 ----
{
  const sep17 = d(2026, 9, 17);
  const holiday: DayItem = { ...ev('holiday', d(2026, 9, 17), undefined, true), calendarId: HOLIDAY_CALENDAR };
  // 구글 종일 일정: 9/17 하루짜리의 end는 9/18 자정
  const allDay17 = ev('allDay17', d(2026, 9, 17), d(2026, 9, 18), true);
  const allDay18 = ev('allDay18', d(2026, 9, 18), d(2026, 9, 19), true);
  // 구글 할 일: 00:00으로 와도 날짜만 유효
  const dateTask = task('dateTask', d(2026, 9, 17), true);
  const timedTask = task('timedTask', d(2026, 9, 17, 14), false);
  const late = ev('late', d(2026, 9, 17, 19), d(2026, 9, 17, 20));
  const early = ev('early', d(2026, 9, 17, 9), d(2026, 9, 17, 10));
  // 자정에 시작하지만 시각이 있는 일정 — 종일로 오판하면 안 됨
  const midnight = ev('midnight', d(2026, 9, 17, 0), d(2026, 9, 17, 1));
  const overnight = ev('overnight', d(2026, 9, 16, 23), d(2026, 9, 17, 1));

  const items = [late, dateTask, allDay18, holiday, timedTask, early, allDay17, midnight, overnight];
  const g = dayGroups(items, sep17);
  const ids = (xs: DayItem[]) => xs.map((x) => x.id).join();

  ok(ids(g.holidays) === 'holiday', '공휴일 묶음');
  ok(ids(g.allDay) === 'allDay17', '종일: 끝이 다음날 자정이어도 하루만, 다음날 일정은 안 들어옴');
  ok(ids(g.tasks) === 'dateTask,timedTask', '할 일은 시각 유무 상관없이 전부');
  ok(ids(g.timed) === 'overnight,midnight,early,timedTask,late', '시각 있는 것만 시작 시각 순');
  ok(g.timed.includes(timedTask) && g.tasks.includes(timedTask), '시각 있는 할 일은 양쪽에 같은 객체');
  ok(!g.timed.includes(dateTask), '00:00 할 일을 시각 있음으로 오판하지 않음');

  const g18 = dayGroups(items, d(2026, 9, 18));
  ok(ids(g18.allDay) === 'allDay18', '다음날에는 전날 종일 일정이 번지지 않음');
  ok(g18.timed.length === 0 && g18.tasks.length === 0, '다음날 나머지 비어 있음');
}

console.log('ok');
