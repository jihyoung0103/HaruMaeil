<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import MonthGrid from '$lib/MonthGrid.svelte';
  import { monthCells, type DayItem } from '$lib/calendar';
  import { listRange, ITEMS_CHANGED } from '$lib/db';
  import { prefs } from '$lib/prefs.svelte';

  const now = new Date();
  const year = now.getFullYear();
  const month = now.getMonth() + 1;

  let items = $state<DayItem[]>([]);
  // 주 시작 설정은 메인 창에서 바꾸면 storage 이벤트로 여기도 바뀐다
  const cells = $derived(monthCells(year, month, prefs.weekStart));

  async function reload() {
    try {
      items = await listRange(cells[0], cells[41]);
    } catch (e) {
      console.error('위젯 조회 실패', e);
    }
  }

  $effect(() => {
    reload();
    const un = listen(ITEMS_CHANGED, reload);
    return () => void un.then((f) => f());
  });
</script>

<!-- 위치 조정 모드에서만 입력이 들어온다. 평소엔 벽지 레이어라 클릭 자체가 안 옴 -->
<div class="widget" data-tauri-drag-region>
  <h1 data-tauri-drag-region>{year}년 {month}월</h1>
  <!-- 격자가 드래그를 먹지 않게. 어차피 위젯은 보기 전용 -->
  <div class="view-only"><MonthGrid {year} {month} {items} weekStart={prefs.weekStart} compact /></div>
</div>

<style>
  :global(html, body) {
    margin: 0;
    background: transparent;
    color-scheme: dark;
  }

  .widget {
    height: 100vh;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr); /* 0 없으면 격자가 min-content 밑으로 안 줄어듦 */
    padding: 0.5rem;
    box-sizing: border-box;
    color: #fff;
    background: rgba(20, 20, 22, 0.45);
    border-radius: 10px;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.6);
    cursor: move;
    /* 셀 배경을 비워서 아래 반투명 카드가 그대로 비치게 */
    --cell-bg: transparent;
    --grid-line: rgba(255, 255, 255, 0.18);
  }

  .view-only {
    pointer-events: none;
    min-height: 0;
  }

  h1 {
    font-size: 0.9rem;
    margin: 0 0 0.35rem;
    text-align: center;
  }
</style>
