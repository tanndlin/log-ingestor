<script lang="ts">
  const LEVELS = ["Fatal", "Error", "Warn", "Info", "Debug", "Trace"] as const;
  type Level = (typeof LEVELS)[number];

  type Trace = {
    level: Level;
    timestamp: string;
    message: string;
  };

  let level = $state<Level | "">("");
  let message = $state("");
  let after = $state("");
  let before = $state("");

  let traces = $state<Trace[]>([]);
  let loading = $state(false);
  let error = $state("");

  async function search(params: URLSearchParams, signal: AbortSignal) {
    loading = true;
    error = "";
    try {
      const res = await fetch(`/api/search?${params}`, { signal });
      if (!res.ok) throw new Error(`Querier responded ${res.status}`);
      const data: Trace[] = await res.json();
      traces = data.sort((a, b) => b.timestamp.localeCompare(a.timestamp));
    } catch (e) {
      if (signal.aborted) return;
      error = e instanceof Error ? e.message : String(e);
    }
    loading = false;
  }

  // Re-query whenever a filter changes. Typing is debounced, and a newer
  // search aborts the one still in flight so stale results never land.
  $effect(() => {
    const params = new URLSearchParams();
    if (level) params.set("level", level);
    if (message) params.set("message", message);
    // datetime-local values are local time; the querier expects RFC 3339.
    if (after) params.set("after", new Date(after).toISOString());
    if (before) params.set("before", new Date(before).toISOString());

    const controller = new AbortController();
    const timer = setTimeout(() => search(params, controller.signal), 250);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  });
</script>

<main>
  <form onsubmit={(e) => e.preventDefault()}>
    <label>
      Level
      <select bind:value={level}>
        <option value="">Any</option>
        {#each LEVELS as l (l)}
          <option value={l}>{l}</option>
        {/each}
      </select>
    </label>
    <label class="grow">
      Message
      <input type="search" placeholder="Contains…" bind:value={message} />
    </label>
    <label>
      After
      <input type="datetime-local" bind:value={after} />
    </label>
    <label>
      Before
      <input type="datetime-local" bind:value={before} />
    </label>
  </form>

  {#if error}
    <p class="status error">{error}</p>
  {:else if !loading && traces.length === 0}
    <p class="status">No logs found.</p>
  {:else}
    <p class="status">{loading ? "Searching…" : `${traces.length} results`}</p>
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Time</th>
            <th>Level</th>
            <th>Message</th>
          </tr>
        </thead>
        <tbody>
          {#each traces as trace, i (i)}
            <tr>
              <td class="time">{new Date(trace.timestamp).toLocaleString()}</td>
              <td><span class="level level-{trace.level.toLowerCase()}">{trace.level}</span></td>
              <td class="message">{trace.message}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</main>

<style>
  :global(:root) {
    --bg: #ffffff;
    --fg: #1a1a1a;
    --muted: #5f6368;
    --border: #e3e3e3;
    --accent: #2563eb;
    --accent-fg: #ffffff;
    --fatal: #9333ea;
    --error: #dc2626;
    --warn: #d97706;
    --info: #2563eb;
    --debug: #5f6368;
    color-scheme: light dark;
    font-family: system-ui, sans-serif;
  }

  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --bg: #121212;
      --fg: #ededed;
      --muted: #a0a0a0;
      --border: #2c2c2c;
      --accent: #60a5fa;
      --accent-fg: #0b0b0b;
      --fatal: #c084fc;
      --error: #f87171;
      --warn: #fbbf24;
      --info: #60a5fa;
      --debug: #a0a0a0;
    }
  }

  :global(body) {
    margin: 0;
    background: var(--bg);
    color: var(--fg);
  }

  main {
    max-width: 1200px;
    margin: 0 auto;
    padding: 16px;
  }

  form {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: end;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.875rem;
    color: var(--muted);
  }

  .grow {
    flex: 1 1 200px;
  }

  input,
  select {
    font: inherit;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--fg);
  }

  .status {
    color: var(--muted);
    font-size: 0.875rem;
  }

  .error {
    color: var(--error);
  }

  .table-wrap {
    overflow-x: auto;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.875rem;
  }

  th,
  td {
    text-align: left;
    padding: 8px;
    border-bottom: 1px solid var(--border);
    vertical-align: top;
  }

  th {
    color: var(--muted);
    font-weight: 600;
  }

  .time {
    white-space: nowrap;
    color: var(--muted);
  }

  .message {
    font-family: ui-monospace, monospace;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .level {
    font-weight: 600;
  }

  .level-fatal {
    color: var(--fatal);
  }
  .level-error {
    color: var(--error);
  }
  .level-warn {
    color: var(--warn);
  }
  .level-info {
    color: var(--info);
  }
  .level-debug,
  .level-trace {
    color: var(--debug);
  }
</style>
