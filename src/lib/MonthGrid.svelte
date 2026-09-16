<script lang="ts">
  import {
    monthCells,
    ymd,
    itemDate,
    layoutWeek,
    hiddenPerDay,
    HOLIDAY_CALENDAR,
    DEFAULT_COLOR,
    type DayItem,
    type WeekStart
  } from './calendar';

  let {
    year,
    month,
    items = [],
    compact = false,
    selected = null,
    weekStart = 0,
    onDayClick
  }: {
    year: number;
    month: number;
    items?: DayItem[];
    compact?: boolean;
    selected?: Date | null;
    weekStart?: WeekStart;
    onDayClick?: (date: Date) => void;
  } = $props();

  const NAMES = ['일', '월', '화', '수', '목', '금', '토'];
  const weekdays = $derived(Array.from({ length: 7 }, (_, i) => (i + weekStart) % 7));
  const today = ymd(new Date());

  // 한 주 줄에 막대가 몇 줄 들어가는지 — 주 줄 높이, 날짜 숫자 자리, 막대 한 줄 높이를 실제로 잰다.
  // 글자 크기가 폭(cqw)에 따라 바뀌어서 상수로 어림하면 막대가 잘리고 "+n"이 사라진다.
  let weekH = $state(0);
  let numH = $state(0);
  let laneH = $state(0);
  const fit = $derived(laneH ? Math.max(1, Math.floor((weekH - numH) / laneH)) : 1);

  /** 다 들어가면 전부 보이고, 넘치면 마지막 한 줄을 "+n" 자리로 비운다 */
  const lanesFor = (weekBars: { lane: number }[]) => {
    const depth = weekBars.reduce((m, b) => Math.max(m, b.lane + 1), 0);
    return depth <= fit ? fit : Math.max(1, fit - 1);
  };

  const selectedKey = $derived(selected ? ymd(selected) : null);
  // 자정이면 시각을 안 붙인다 (종일 일정으로 취급)
  const hhmm = (d: Date) =>
    d.getHours() || d.getMinutes() ? `${d.getHours()}:${String(d.getMinutes()).padStart(2, '0')} ` : '';

  const cells = $derived(monthCells(year, month, weekStart));
  const weeks = $derived(Array.from({ length: 6 }, (_, w) => cells.slice(w * 7, w * 7 + 7)));
  const bars = $derived(weeks.map((days) => layoutWeek(days, items)));
  // 공휴일은 날짜 숫자도 빨갛게
  const holidays = $derived(
    new Set(items.filter((i) => i.calendarId === HOLIDAY_CALENDAR).map((i) => ymd(itemDate(i)!)))
  );
  const inMonth = (d: Date) => d.getMonth() === month - 1;
</script>

<div class="grid" class:compact>
  <!-- 크기 재기용. 절대 위치라 격자 칸을 차지하지 않는다 -->
  <span class="probe" style:height="var(--num-h)" bind:clientHeight={numH}></span>
  <span class="probe" style:height="var(--lane-h)" bind:clientHeight={laneH}></span>

  <div class="head">
    {#each weekdays as dow}
      <div class:sun={dow === 0} class:sat={dow === 6}>{NAMES[dow]}</div>
    {/each}
  </div>

  {#each weeks as days, w}
    {@const lanes = lanesFor(bars[w])}
    {@const hidden = hiddenPerDay(bars[w], lanes)}
    <!-- 날짜 칸은 모든 줄을 덮는 바닥, 막대는 그 위에 칸을 가로질러 얹힌다 -->
    <div
      class="week"
      bind:clientHeight={weekH}
      style:grid-template-rows="var(--num-h) repeat({lanes + 1}, var(--lane-h)) 1fr"
    >
      {#each days as date, c (date.getTime())}
        {@const key = ymd(date)}
        <button
          class="cell"
          style:grid-column={c + 1}
          class:other={!inMonth(date)}
          class:today={key === today}
          class:sel={key === selectedKey}
          class:sun={date.getDay() === 0 || holidays.has(key)}
          class:sat={date.getDay() === 6 && !holidays.has(key)}
          onclick={() => onDayClick?.(date)}
        >
          <span class="num">{date.getDate()}</span>
        </button>
      {/each}

      {#each bars[w] as b (b.item.id)}
        {#if b.lane < lanes}
          <span
            class="bar"
            class:done={b.item.done}
            class:cont-l={b.startsBefore}
            class:cont-r={b.endsAfter}
            class:other={!inMonth(days[b.col]) && !inMonth(days[b.col + b.span - 1])}
            style:grid-column="{b.col + 1} / span {b.span}"
            style:grid-row={b.lane + 2}
            style:--c={b.item.color ?? DEFAULT_COLOR}
            title={b.item.title}
            >{#if b.item.start && !b.startsBefore}<b>{hhmm(b.item.start)}</b>{/if}{b.item.title}</span
          >
        {/if}
      {/each}

      {#each hidden as n, c}
        {#if n > 0}
          <span class="more" style:grid-column={c + 1} style:grid-row={lanes + 2}>+{n}</span>
        {/if}
      {/each}
    </div>
  {/each}
</div>

<style>
  .grid {
    position: relative;
    display: grid;
    /* 칸 높이는 최소값만 주고 남는 높이는 나눠 가짐 — 창 높이가 작으면 알아서 줄어듦 */
    grid-template-rows: auto repeat(6, minmax(var(--cell-min, 5.5rem), 1fr));
    height: 100%;
    /* 글자 크기를 격자 너비에 맞춰 스케일 (아래 cqw) */
    container-type: inline-size;
    box-sizing: border-box;
    gap: 1px;
    background: var(--grid-line, color-mix(in srgb, currentColor 15%, transparent));
    border: 1px solid var(--grid-line, color-mix(in srgb, currentColor 15%, transparent));
    --num-size: clamp(0.66rem, 1.85cqw, 0.95rem);
    --bar-size: clamp(0.55rem, 1.5cqw, 0.75rem);
    --num-h: calc(var(--num-size) * 1.45 + 0.3rem);
    --lane-h: calc(var(--bar-size) * 1.45 + 2px);
  }
  .compact {
    --cell-min: 1.9rem;
  }
  .probe {
    position: absolute;
    visibility: hidden;
    pointer-events: none;
  }

  .head,
  .week {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    column-gap: 1px;
  }
  /* 주 줄만 줄어들 수 있게. 머리글에 주면 격자가 넘칠 때 머리글 행이 0으로 접혀 첫 주와 겹친다 */
  .week {
    min-height: 0;
    overflow: hidden;
  }

  .head > div {
    background: var(--cell-bg, Canvas);
    padding: 0.35rem 0;
    text-align: center;
    font-size: clamp(0.62rem, 1.7cqw, 0.85rem);
    opacity: 0.7;
  }

  .cell {
    grid-row: 1 / -1;
    background: var(--cell-bg, Canvas);
    color: inherit;
    border: 0;
    font: inherit;
    text-align: left;
    padding: 0.15rem 0.2rem;
    display: flex;
    align-items: flex-start;
    cursor: pointer;
  }
  .cell:hover,
  .cell:focus-visible {
    background: color-mix(in srgb, currentColor 20%, transparent);
  }

  /* 칸 전체에 opacity를 주면 격자선이 비쳐서 오히려 밝아짐 — 내용만 흐리게 */
  .cell.other .num,
  .bar.other {
    opacity: 0.4;
  }
  .sun {
    color: #d33;
  }
  .sat {
    color: #36c;
  }

  .num {
    font-size: var(--num-size);
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

  .bar {
    /* 클릭은 밑의 날짜 칸이 받는다 */
    pointer-events: none;
    margin: 1px 3px;
    padding: 0 0.3rem;
    border-radius: 3px;
    font-size: var(--bar-size);
    line-height: calc(var(--lane-h) - 2px);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    background: color-mix(in srgb, var(--c) 45%, transparent);
    color: inherit;
  }
  /* 앞뒤 주로 이어지는 쪽은 끝을 평평하게 붙인다 */
  .bar.cont-l {
    margin-left: 0;
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
  }
  .bar.cont-r {
    margin-right: 0;
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
  }
  .bar b {
    font-weight: 400;
    opacity: 0.75;
  }
  .bar.done {
    text-decoration: line-through;
    opacity: 0.5;
  }
  .more {
    pointer-events: none;
    font-size: clamp(0.52rem, 1.35cqw, 0.7rem);
    line-height: var(--lane-h);
    opacity: 0.6;
    padding: 0 0.4rem;
  }
</style>
