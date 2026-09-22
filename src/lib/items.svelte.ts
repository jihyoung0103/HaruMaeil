import type { DayItem } from './calendar';
import { listRange } from './db';

/**
 * 한 창 안의 일정·할 일 원본. 월 격자와 사이드 패널이 이 배열 안의 **같은 객체**를 참조한다 —
 * 컴포넌트 안으로 복사하지 말 것. 복사하면 한쪽에서 완료 처리한 게 다른 쪽에 안 보인다.
 * 메인 창과 위젯은 서로 다른 웹뷰라 각자 이 모듈을 따로 갖고, 둘 사이는 DB + ITEMS_CHANGED 이벤트로 맞춘다.
 */
export const itemStore = $state({ items: [] as DayItem[] });

/** from~to(양끝 포함)를 DB에서 다시 읽어 원본을 갈아끼운다 */
export async function loadItems(from: Date, to: Date): Promise<void> {
  itemStore.items = await listRange(from, to);
}
