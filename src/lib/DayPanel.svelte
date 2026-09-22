<script lang="ts">
  import { dayGroups, hhmm, itemDate, itemDays, DEFAULT_COLOR, type DayItem } from './calendar';

  // items는 원본(itemStore.items)을 그대로 받는다. 묶음은 $derived로 걸러낼 뿐 복사하지 않는다
  let { day, items }: { day: Date; items: DayItem[] } = $props();

  const DOW = ['일요일', '월요일', '화요일', '수요일', '목요일', '금요일', '토요일'];

  const g = $derived(dayGroups(items, day));
  // 구분선 위 = "오늘이 어떤 날인가", 아래 = "오늘 몇 시에 뭘 하나"
  const hasUpper = $derived(g.holidays.length + g.allDay.length + g.tasks.length > 0);

  const color = (i: DayItem) => i.color ?? i.calendarColor ?? DEFAULT_COLOR;
  // 전날부터 이어지는 시각 있는 일정은 시작 시각이 오늘이 아니다
  const startsBefore = (i: DayItem) =>
    itemDays(i)![0] < new Date(day.getFullYear(), day.getMonth(), day.getDate());
</script>

<aside class="day-panel">
  <!-- 1. 날짜 -->
  <header
    class:red={day.getDay() === 0 || g.holidays.length > 0}
    class:blue={day.getDay() === 6 && g.holidays.length === 0}
  >
    <span class="num">{day.getDate()}</span>
    <span class="dow">{DOW[day.getDay()]}</span>
  </header>

  <!-- 2. 공휴일·기념일 -->
  {#if g.holidays.length}
    <ul class="holidays">
      {#each g.holidays as i (i.id)}
        <li>{i.title}</li>
      {/each}
    </ul>
  {/if}

  <!-- 3. 종일 일정 -->
  {#if g.allDay.length}
    <ul>
      {#each g.allDay as i (i.id)}
        <li class="row" style:--c={color(i)}>{i.title}</li>
      {/each}
    </ul>
  {/if}

  <!-- 4. 그날 할 일 전부. 시각 있는 건 뒤에 회색으로 붙이고 6번에도 한 번 더 나온다 -->
  {#if g.tasks.length}
    <ul>
      {#each g.tasks as i (i.id)}
        <li class="row task" class:done={i.done} style:--c={color(i)}>
          <span class="box" class:checked={i.done} aria-hidden="true"></span>
          <span>{i.title}{#if !i.allDay}<span class="time">{' · ' + hhmm(itemDate(i)!)}</span>{/if}</span>
        </li>
      {/each}
    </ul>
  {/if}

  <!-- 5. 구분선 — 위아래 둘 다 있을 때만 -->
  {#if hasUpper && g.timed.length}
    <hr />
  {/if}

  <!-- 6. 시각 있는 일정 + 시각 있는 할 일, 시작 시각 순 -->
  {#if g.timed.length}
    <ol class="stack">
      {#each g.timed as i (i.id)}
        <li class="row" class:done={i.done} style:--c={color(i)}>
          <span class="time">{startsBefore(i) ? '이어서' : hhmm(itemDate(i)!)}</span>
          <span>{i.title}</span>
        </li>
      {/each}
    </ol>
  {/if}

  {#if !hasUpper && !g.timed.length}
    <p class="empty">일정 없음</p>
  {/if}
</aside>

<style>
  .day-panel {
    height: 100%;
    box-sizing: border-box;
    padding: 0.6rem 0.7rem;
    overflow: auto;
    background: var(--panel-bg, transparent);
    border-radius: 8px;
    font-size: 0.85rem;
    line-height: 1.4;
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
    margin-bottom: 0.5rem;
  }
  .num {
    font-size: 2rem;
    font-weight: 700;
    line-height: 1;
  }
  .dow {
    opacity: 0.8;
  }
  .red {
    color: #e5534b;
  }
  .blue {
    color: #4d8ce8;
  }

  ul,
  ol {
    list-style: none;
    margin: 0 0 0.5rem;
    padding: 0;
  }
  .holidays li {
    color: #e5534b;
    font-weight: 600;
  }

  /* 캘린더 색을 왼쪽 띠로 */
  .row {
    display: flex;
    align-items: baseline;
    gap: 0.4rem;
    padding: 0.1rem 0 0.1rem 0.45rem;
    margin-bottom: 0.2rem;
    border-left: 3px solid var(--c);
  }

  .task .box {
    flex-shrink: 0;
    width: 0.7rem;
    height: 0.7rem;
    border: 1.5px solid currentColor;
    border-radius: 3px;
    opacity: 0.7;
    align-self: center;
  }
  .task .box.checked {
    background: currentColor;
  }

  .time {
    opacity: 0.6;
  }
  .stack .time {
    flex-shrink: 0;
    min-width: 2.6rem;
    font-variant-numeric: tabular-nums;
  }

  .done {
    text-decoration: line-through;
    opacity: 0.55;
  }

  hr {
    border: 0;
    border-top: 1px solid color-mix(in srgb, currentColor 25%, transparent);
    margin: 0.6rem 0;
  }

  .empty {
    margin: 0;
    opacity: 0.5;
  }
</style>
