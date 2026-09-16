import type { WeekStart } from './calendar';

/**
 * 화면 설정. 메인 창과 위젯은 같은 출처라 localStorage를 공유하고, 한쪽에서 바꾸면
 * 다른 창에 storage 이벤트가 온다 — 창 사이 동기화를 따로 만들 필요가 없다.
 */
const WEEK_START = 'harumaeil.weekStart';

function readWeekStart(): WeekStart {
  try {
    return localStorage.getItem(WEEK_START) === '1' ? 1 : 0;
  } catch {
    return 0;
  }
}

export const prefs = $state({ weekStart: readWeekStart() });

export function setWeekStart(v: WeekStart) {
  prefs.weekStart = v;
  try {
    localStorage.setItem(WEEK_START, String(v));
  } catch {
    // 저장 못 해도 이번 실행에서는 적용된다
  }
}

window.addEventListener('storage', (e) => {
  if (e.key === WEEK_START) prefs.weekStart = readWeekStart();
});
