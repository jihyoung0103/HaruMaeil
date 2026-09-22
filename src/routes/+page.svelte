<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import MonthGrid from '$lib/MonthGrid.svelte';
  import DayPanel from '$lib/DayPanel.svelte';
  import {
    monthCells,
    coversDay,
    LOCAL_CALENDAR,
    HOLIDAY_CALENDAR,
    type DayItem,
    type WeekStart
  } from '$lib/calendar';
  import { saveItem, deleteItem, ITEMS_CHANGED } from '$lib/db';
  import { itemStore, loadItems } from '$lib/items.svelte';
  import { ensureHolidays } from '$lib/holidays';
  import { prefs, setWeekStart } from '$lib/prefs.svelte';
  import {
    googleStatus,
    googleConnect,
    googleDisconnect,
    syncGoogle,
    type GoogleStatus
  } from '$lib/google';

  const now = new Date();
  let year = $state(now.getFullYear());
  let month = $state(now.getMonth() + 1);

  let selected = $state<Date | null>(null);
  let error = $state('');
  let holidayError = $state('');

  // 격자가 그리는 42칸. 주 시작 요일에 따라 앞뒤 하루씩 달라진다
  const cells = $derived(monthCells(year, month, prefs.weekStart));

  let draft = $state('');
  let draftKind = $state<'event' | 'task'>('event');
  let draftTime = $state('');

  // 사이드 패널은 지금 오늘을 보여준다. 다른 달로 넘겨도 오늘 항목이 원본에 남도록 읽는 구간에 오늘을 포함한다
  // ponytail: 자정을 넘기면 now가 안 바뀐다 — P1-5(15분 타이머)에서 같이 처리
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());

  async function reload() {
    try {
      await loadItems(cells[0] < today ? cells[0] : today, cells[41] > today ? cells[41] : today);
      error = '';
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    reload();
    // 공휴일은 구글 연결과 상관없이 받는다
    ensureHolidays(cells[0], cells[41]).then(
      () => (holidayError = ''),
      (e) => (holidayError = String(e))
    );
  });

  // 다른 창(위젯)이 쓰거나 내가 쓰거나, 바뀌면 다시 읽는다
  $effect(() => {
    const un = listen(ITEMS_CHANGED, reload);
    return () => void un.then((f) => f());
  });

  // 여러 날 일정은 가운데 날짜를 눌러도 나온다
  const dayItems = $derived(
    selected ? itemStore.items.filter((i) => coversDay(i, selected!)) : []
  );

  async function add() {
    if (!selected || !draft.trim()) return;
    const [h, m] = draftTime ? draftTime.split(':').map(Number) : [0, 0];
    const when = new Date(selected.getFullYear(), selected.getMonth(), selected.getDate(), h, m);
    const base = {
      id: crypto.randomUUID(),
      title: draft,
      calendarId: LOCAL_CALENDAR,
      // 할 일은 시각 입력칸이 없어서 항상 종일, 일정은 시각을 비우면 종일
      allDay: draftKind === 'task' || !draftTime
    };
    try {
      await saveItem(
        draftKind === 'task'
          ? { ...base, kind: 'task', due: when, done: false }
          : { ...base, kind: 'event', start: when }
      );
      draft = '';
      draftTime = '';
    } catch (e) {
      error = String(e);
    }
  }

  async function toggleDone(item: DayItem) {
    try {
      await saveItem({ ...item, done: !item.done });
    } catch (e) {
      error = String(e);
    }
  }

  async function remove(id: string) {
    try {
      await deleteItem(id);
    } catch (e) {
      error = String(e);
    }
  }

  function moveMonth(delta: number) {
    const d = new Date(year, month - 1 + delta, 1);
    year = d.getFullYear();
    month = d.getMonth() + 1;
  }

  function goToday() {
    const d = new Date();
    year = d.getFullYear();
    month = d.getMonth() + 1;
  }

  // ---- 구글 연동 ----
  let gs = $state<GoogleStatus>({ hasClientId: false, connected: false });
  let googleMsg = $state('');
  let busy = $state(false);

  const refreshStatus = async () => (gs = await googleStatus());
  $effect(() => void refreshStatus());

  async function run(label: string, fn: () => Promise<string>) {
    busy = true;
    googleMsg = `${label} 중…`;
    try {
      googleMsg = await fn();
    } catch (e) {
      googleMsg = String(e);
    }
    await refreshStatus();
    busy = false;
  }

  // 마지막 동기화 — 앱을 다시 켜도 보이게 저장해둔다
  const LAST_SYNC = 'harumaeil.lastSync';
  type LastSync = { at: number; events: number; tasks: number };
  let lastSync = $state<LastSync | null>(
    (() => {
      try {
        return JSON.parse(localStorage.getItem(LAST_SYNC) ?? 'null');
      } catch {
        return null;
      }
    })()
  );
  function recordSync(n: { events: number; tasks: number } | null) {
    lastSync = n && { at: Date.now(), ...n };
    clock = Date.now();
    try {
      if (lastSync) localStorage.setItem(LAST_SYNC, JSON.stringify(lastSync));
      else localStorage.removeItem(LAST_SYNC);
    } catch {
      // 저장 못 해도 이번 실행에서는 보인다
    }
  }

  // "N분 전"이 시간이 지나면 저절로 바뀌게
  let clock = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (clock = Date.now()), 30_000);
    return () => clearInterval(t);
  });
  const rtf = new Intl.RelativeTimeFormat('ko', { numeric: 'auto' });
  function ago(at: number) {
    const s = (clock - at) / 1000;
    if (s < 60) return '방금';
    if (s < 3600) return rtf.format(-Math.floor(s / 60), 'minute');
    if (s < 86400) return rtf.format(-Math.floor(s / 3600), 'hour');
    return rtf.format(-Math.floor(s / 86400), 'day'); // 어제, 그저께, N일 전
  }

  // 연결 직후 바로 한 번 가져온다. 안 그러면 연결했는데 화면이 그대로라 실패한 줄 안다
  const connect = () =>
    run('연결', async () => {
      await googleConnect();
      recordSync(await syncGoogle(cells[0], cells[41]));
      return '';
    });

  const disconnect = () =>
    run('해제', async () => {
      await googleDisconnect();
      recordSync(null);
      return '연결을 해제했습니다.';
    });

  const sync = () =>
    run('동기화', async () => {
      recordSync(await syncGoogle(cells[0], cells[41]));
      return ''; // 결과는 "마지막 동기화" 줄이 보여준다
    });

  // ---- 창 버튼 ----
  // 기본 타이틀 바를 껐으니 헤더가 대신한다. 최대화 상태에 따라 □/❐ 아이콘을 바꾼다
  const appWindow = getCurrentWindow();
  let maximized = $state(false);
  $effect(() => {
    const check = async () => (maximized = await appWindow.isMaximized());
    check();
    const un = appWindow.onResized(check);
    return () => void un.then((f) => f());
  });

  // 위젯은 벽지 레이어에 있어 입력을 못 받는다. 옮기려면 잠깐 떼어내야 함
  let editingWidget = $state(false);
  let widgetError = $state('');
  async function toggleWidgetEdit() {
    try {
      await invoke('widget_edit', { on: !editingWidget });
      editingWidget = !editingWidget;
      widgetError = '';
    } catch (e) {
      widgetError = String(e);
    }
  }
</script>

<main>
  <!-- 헤더가 타이틀 바를 겸한다: 빈 곳을 잡고 창 이동, 더블클릭으로 최대화.
       Tauri는 속성이 붙은 요소 자체를 잡았을 때만 끌기 때문에 제목에도 따로 붙인다 -->
  <header data-tauri-drag-region>
    <button onclick={() => moveMonth(-1)} aria-label="이전 달">‹</button>
    <h1 data-tauri-drag-region>{year}년 {month}월</h1>
    <button onclick={() => moveMonth(1)} aria-label="다음 달">›</button>
    <button class="today" onclick={goToday}>오늘</button>
    <!-- ponytail: 설정 화면(TODO P5)이 생기면 그리로 옮길 것 -->
    <select
      value={prefs.weekStart}
      onchange={(e) => setWeekStart(+e.currentTarget.value as WeekStart)}
      aria-label="주 시작 요일"
    >
      <option value={0}>일요일 시작</option>
      <option value={1}>월요일 시작</option>
    </select>
    <button class:editing={editingWidget} onclick={toggleWidgetEdit}>
      {editingWidget ? '위치 저장' : '위젯 위치'}
    </button>

    <!-- 아이콘은 윈도우가 자기 창 버튼에 쓰는 글꼴 그대로 (Win11 Segoe Fluent Icons, Win10 Segoe MDL2 Assets) -->
    <div class="caption">
      <button onclick={() => appWindow.minimize()} aria-label="최소화">&#xE921;</button>
      <button onclick={() => appWindow.toggleMaximize()} aria-label={maximized ? '이전 크기로' : '최대화'}
        >{maximized ? '\uE923' : '\uE922'}</button
      >
      <button class="close" onclick={() => appWindow.close()} aria-label="닫기">&#xE8BB;</button>
    </div>
  </header>

  {#if error}<p class="err">{error}</p>{/if}
  {#if widgetError}<p class="err">위젯: {widgetError}</p>{/if}
  {#if holidayError}<p class="err">공휴일: {holidayError}</p>{/if}

  <details class="google">
    <summary>구글 캘린더 {gs.connected ? '· 연결됨' : gs.hasClientId ? '· 미연결' : '· 설정 필요'}</summary>

    {#if !gs.hasClientId}
      <p class="hint">
        이 빌드에 구글 자격증명이 없습니다. <code>src-tauri/google-client.json</code>을 넣고
        <code>npm run tauri dev</code>를 다시 켜세요.
      </p>
    {/if}

    <div class="row">
      <button onclick={connect} disabled={busy || !gs.hasClientId}>
        {gs.connected ? '다시 연결' : '연결하기'}
      </button>
      <button onclick={sync} disabled={busy || !gs.connected}>이 달 가져오기</button>
      <button onclick={disconnect} disabled={busy || !gs.connected}>연결 해제</button>
    </div>

    {#if lastSync}
      <p class="hint">
        마지막 동기화: {ago(lastSync.at)} (일정 {lastSync.events}건, 할 일 {lastSync.tasks}건 완료)
      </p>
    {/if}
    {#if googleMsg}<p class="hint">{googleMsg}</p>{/if}
  </details>

  <div class="body">
    <!-- 격자는 창 높이 안에 가둔다. 높이가 내용을 따라가면 막대 줄 수 계산이 스스로를 키운다 -->
    <div class="grid-wrap">
      <MonthGrid
        {year}
        {month}
        items={itemStore.items}
        {selected}
        weekStart={prefs.weekStart}
        onDayClick={(d) => (selected = d)}
      />
    </div>
    <!-- 이번 단계는 오늘 고정. 월 격자에서 날짜 누르면 바뀌게 하는 건 다음 단계 -->
    <div class="panel-wrap">
      <DayPanel day={today} items={itemStore.items} />
    </div>
  </div>

  {#if selected}
    <section class="editor">
      <h2>{selected.getMonth() + 1}월 {selected.getDate()}일</h2>

      <ul>
        {#each dayItems as item (item.id)}
          <li>
            {#if item.kind === 'task'}
              <input
                type="checkbox"
                checked={item.done}
                disabled={item.calendarId !== LOCAL_CALENDAR}
                onchange={() => toggleDone(item)}
                aria-label="완료"
              />
            {/if}
            <span class:done={item.done}>{item.title}</span>
            {#if item.calendarId === LOCAL_CALENDAR}
              <button class="del" onclick={() => remove(item.id)} aria-label="삭제">✕</button>
            {:else}
              <!-- 여기서 지워도 다음 동기화 때 다시 생긴다. 구글 쪽 수정은 양방향(P3)에서 -->
              <span class="src" title="가져온 항목은 아직 여기서 수정할 수 없습니다"
                >{item.calendarId === HOLIDAY_CALENDAR ? '공휴일' : '구글'}</span
              >
            {/if}
          </li>
        {:else}
          <li class="empty">없음</li>
        {/each}
      </ul>

      <form onsubmit={(e) => (e.preventDefault(), add())}>
        <select bind:value={draftKind}>
          <option value="event">일정</option>
          <option value="task">할 일</option>
        </select>
        {#if draftKind === 'event'}
          <input type="time" bind:value={draftTime} />
        {/if}
        <input placeholder="제목" bind:value={draft} />
        <button type="submit">추가</button>
      </form>
    </section>
  {/if}
</main>

<style>
  :global(:root) {
    color-scheme: light dark;
  }
  :global(body) {
    margin: 0;
  }

  main {
    padding: 0.75rem;
    height: 100vh;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    gap: 0.75rem;
  }
  .grid-wrap {
    flex: 1;
    min-width: 0;
    overflow: auto;
    /* 기본 800×600 창에서도 6주가 스크롤 없이 들어가게 */
    --cell-min: 3.5rem;
  }

  .panel-wrap {
    width: 15rem;
    flex-shrink: 0;
    min-height: 0;
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    height: 2.75rem;
    /* 창 맨 위·오른쪽 끝까지 붙여서 창 버튼이 모서리에 오게 */
    margin: -0.75rem -0.75rem 0.5rem 0;
    flex-shrink: 0;
  }

  .caption {
    display: flex;
    align-self: stretch;
    margin-left: 0.4rem;
  }
  .caption button {
    width: 46px;
    border: 0;
    border-radius: 0;
    padding: 0;
    font-family: 'Segoe Fluent Icons', 'Segoe MDL2 Assets', sans-serif;
    font-size: 10px;
  }
  .caption .close:hover {
    background: #c42b1c;
    color: #fff;
  }

  h1 {
    font-size: 1.1rem;
    margin: 0 0.3rem;
  }

  button,
  input,
  select {
    font: inherit;
    color: inherit;
    background: none;
    border: 1px solid color-mix(in srgb, currentColor 25%, transparent);
    border-radius: 6px;
    padding: 0.15rem 0.5rem;
  }
  button {
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    background: color-mix(in srgb, currentColor 10%, transparent);
  }
  button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .today {
    margin-left: auto;
    font-size: 0.85rem;
  }
  .editing {
    border-color: #d33;
    color: #d33;
  }

  .google {
    margin-bottom: 0.6rem;
    font-size: 0.85rem;
  }
  .google summary {
    cursor: pointer;
    opacity: 0.8;
  }
  .google .row {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.4rem;
  }
  .hint {
    margin: 0.4rem 0 0;
    opacity: 0.7;
    line-height: 1.5;
  }

  .err {
    margin: 0 0 0.5rem;
    font-size: 0.85rem;
    color: #d33;
  }

  .editor {
    margin-top: 0.75rem;
  }
  .editor h2 {
    font-size: 0.95rem;
    margin: 0 0 0.4rem;
  }
  .editor ul {
    list-style: none;
    margin: 0 0 0.5rem;
    padding: 0;
  }
  .editor li {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.15rem 0;
    font-size: 0.9rem;
  }
  .empty {
    opacity: 0.5;
  }
  .done {
    text-decoration: line-through;
    opacity: 0.5;
  }
  .src {
    font-size: 0.75rem;
    opacity: 0.5;
  }
  .del {
    border: 0;
    padding: 0 0.3rem;
    opacity: 0.5;
  }
  .del:hover {
    opacity: 1;
    color: #d33;
  }

  form {
    display: flex;
    gap: 0.4rem;
  }
  form input[placeholder='제목'] {
    flex: 1;
    max-width: 20rem;
  }
</style>
