<script lang="ts">
  import { monthCells, byDay, ymd, type DayItem } from './calendar';

  let {
    year,
    month,
    items = [],
    compact = false,
    selected = null,
    onDayClick
  }: {
    year: number;
    month: number;
    items?: DayItem[];
    compact?: boolean;
    selected?: Date | null;
    onDayClick?: (date: Date) => void;
  } = $props();

  const WEEKDAYS = ['일', '월', '화', '수', '목', '금', '토'];
  const today = ymd(new Date());

  // 칸에 항목이 몇 개 들어가는지를 실제 높이에서 역산 — 창을 키우면 더 보인다.
  // ponytail: 항목/날짜 높이를 상수로 어림잡음. 폰트를 크게 바꾸면 실제 칸을 재도록 고칠 것
  let gridH = $state(0);
  const max = $derived(Math.max(1, Math.min(4, Math.floor(((gridH - 28) / 6 - 19) / 15))));

  const selectedKey = $derived(selected ? ymd(selected) : null);
  // 자정이면 시각을 안 붙인다 (종일 일정으로 취급)
  const hhmm = (d: Date) =>
    d.getHours() || d.getMinutes() ? `${d.getHours()}:${String(d.getMinutes()).padStart(2, '0')} ` : '';

  const cells = $derived(monthCells(year, month));
  const map = $derived(byDay(items));
</script>

<div class="grid" class:compact bind:clientHeight={gridH}>
  {#each WEEKDAYS as w, i}
    <div class="head" class:sun={i === 0} class:sat={i === 6}>{w}</div>
  {/each}

  {#each cells as date (date.getTime())}
    {@const key = ymd(date)}
    {@const day = map.get(key) ?? []}
    <button
      class="cell"
      class:other={date.getMonth() !== month - 1}
      class:today={key === today}
      class:sel={key === selectedKey}
      class:sun={date.getDay() === 0}
      class:sat={date.getDay() === 6}
      onclick={() => onDayClick?.(date)}
    >
      <span class="num">{date.getDate()}</span>
      {#each day.slice(0, max) as item (item.id)}
        <span class="item" class:done={item.done}
          >{#if item.start}<b>{hhmm(item.start)}</b>{/if}{item.title}</span
        >
      {/each}
      {#if day.length > max}
        <span class="more">+{day.length - max}</span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    /* 칸 높이는 최소값만 주고 남는 높이는 나눠 가짐 — 창 높이가 작으면 알아서 줄어듦 */
    grid-template-rows: auto repeat(6, minmax(var(--cell-min, 5.5rem), 1fr));
    height: 100%;
    /* 글자 크기를 격자 너비에 맞춰 스케일 (아래 cqw) */
    container-type: inline-size;
    box-sizing: border-box;
    gap: 1px;
    background: var(--grid-line, color-mix(in srgb, currentColor 15%, transparent));
    border: 1px solid var(--grid-line, color-mix(in srgb, currentColor 15%, transparent));
  }

  .head {
    background: var(--cell-bg, Canvas);
    padding: 0.35rem 0;
    text-align: center;
    font-size: clamp(0.62rem, 1.7cqw, 0.85rem);
    opacity: 0.7;
  }

  .cell {
    background: var(--cell-bg, Canvas);
    color: inherit;
    border: 0;
    font: inherit;
    text-align: left;
    padding: 0.2rem;
    display: flex;
    flex-direction: column;
    gap: 1px;
    overflow: hidden;
    cursor: pointer;
  }
  .compact {
    --cell-min: 1.9rem;
  }

  .cell:hover,
  .cell:focus-visible {
    background: color-mix(in srgb, currentColor 20%, transparent);
  }

  /* 셀 전체에 opacity를 주면 격자선이 비쳐서 오히려 밝아짐 — 내용만 흐리게 */
  .other .num,
  .other .item {
    opacity: 0.4;
  }
  .sun {
    color: #d33;
  }
  .sat {
    color: #36c;
  }

  .num {
    font-size: clamp(0.66rem, 1.85cqw, 0.95rem);
    align-self: flex-start;
    padding: 0 0.25rem;
    border-radius: 999px;
  }
  .today .num {
    background: #d33;
    color: #fff;
  }
  .sel {
    outline: 2px solid #39f;
    outline-offset: -2px;
  }

  .item {
    font-size: clamp(0.55rem, 1.5cqw, 0.75rem);
    line-height: 1.3;
    padding: 0 0.2rem;
    border-radius: 3px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    background: color-mix(in srgb, #36c 25%, transparent);
    color: inherit;
  }
  .item b {
    font-weight: 400;
    opacity: 0.75;
  }
  .item.done {
    text-decoration: line-through;
    opacity: 0.5;
  }
  .more {
    font-size: clamp(0.52rem, 1.35cqw, 0.7rem);
    opacity: 0.6;
    padding: 0 0.2rem;
  }
</style>
