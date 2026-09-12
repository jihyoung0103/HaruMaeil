<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import MonthGrid from '$lib/MonthGrid.svelte';
  import { monthCells, ymd, itemDate, type DayItem } from '$lib/calendar';
  import { listRange, saveItem, deleteItem, ITEMS_CHANGED } from '$lib/db';
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

  let items = $state<DayItem[]>([]);
  let selected = $state<Date | null>(null);
  let error = $state('');

  let draft = $state('');
  let draftKind = $state<'event' | 'task'>('event');
  let draftTime = $state('');

  async function reload() {
    const cells = monthCells(year, month);
    try {
      items = await listRange(cells[0], cells[41]);
      error = '';
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    year;
    month;
    reload();
  });

  // 다른 창(위젯)이 쓰거나 내가 쓰거나, 바뀌면 다시 읽는다
  $effect(() => {
    const un = listen(ITEMS_CHANGED, reload);
    return () => void un.then((f) => f());
  });

  const dayItems = $derived(
    selected ? items.filter((i) => ymd(itemDate(i)!) === ymd(selected!)) : []
  );

  async function add() {
    if (!selected || !draft.trim()) return;
    const [h, m] = draftTime ? draftTime.split(':').map(Number) : [0, 0];
    const when = new Date(selected.getFullYear(), selected.getMonth(), selected.getDate(), h, m);
    const base = { id: crypto.randomUUID(), title: draft, source: 'local' };
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

  // 연결 직후 바로 한 번 가져온다. 안 그러면 연결했는데 화면이 그대로라 실패한 줄 안다
  const connect = () =>
    run('연결', async () => {
      await googleConnect();
      const cells = monthCells(year, month);
      const n = await syncGoogle(cells[0], cells[41]);
      return `연결됐습니다. ${n}건 가져왔습니다.`;
    });

  const disconnect = () =>
    run('해제', async () => {
      await googleDisconnect();
      return '연결을 해제했습니다.';
    });

  const sync = () =>
    run('동기화', async () => {
      const cells = monthCells(year, month);
      const n = await syncGoogle(cells[0], cells[41]);
      return `${n}건 가져왔습니다.`;
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
  <header>
    <button onclick={() => moveMonth(-1)} aria-label="이전 달">‹</button>
    <h1>{year}년 {month}월</h1>
    <button onclick={() => moveMonth(1)} aria-label="다음 달">›</button>
    <button class="today" onclick={goToday}>오늘</button>
    <button class:editing={editingWidget} onclick={toggleWidgetEdit}>
      {editingWidget ? '위치 저장' : '위젯 위치'}
    </button>
  </header>

  {#if error}<p class="err">{error}</p>{/if}
  {#if widgetError}<p class="err">위젯: {widgetError}</p>{/if}

  <details class="google">
    <summary>구글 캘린더 {gs.connected ? '· 연결됨' : gs.hasClientId ? '· 미연결' : '· 설정 필요'}</summary>

    {#if !gs.hasClientId}
      <p class="hint">
        이 빌드에 구글 클라이언트 ID가 없습니다. <code>src-tauri/src/google.rs</code>의
        <code>CLIENT_ID</code>에 값을 넣고 다시 빌드하세요.
      </p>
    {/if}

    <div class="row">
      <button onclick={connect} disabled={busy || !gs.hasClientId}>
        {gs.connected ? '다시 연결' : '연결하기'}
      </button>
      <button onclick={sync} disabled={busy || !gs.connected}>이 달 가져오기</button>
      <button onclick={disconnect} disabled={busy || !gs.connected}>연결 해제</button>
    </div>

    {#if googleMsg}<p class="hint">{googleMsg}</p>{/if}
  </details>

  <MonthGrid {year} {month} {items} {selected} onDayClick={(d) => (selected = d)} />

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
                onchange={() => toggleDone(item)}
                aria-label="완료"
              />
            {/if}
            <span class:done={item.done}>{item.title}</span>
            <button class="del" onclick={() => remove(item.id)} aria-label="삭제">✕</button>
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
    font-family: 'Malgun Gothic', Inter, system-ui, sans-serif;
  }
  :global(body) {
    margin: 0;
  }

  main {
    padding: 0.75rem;
  }

  header {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-bottom: 0.5rem;
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
