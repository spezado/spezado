<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Closing a period, and the data transfer of the GoBD for an auditor. -->
<script lang="ts">
  import { api, download, problem } from "../lib/api";
  import { date } from "../lib/format";
  import { app, t } from "../lib/state.svelte";

  let closed = $state<string | null>(null);
  let failure = $state<string | null>(null);
  let message = $state<string | null>(null);
  const lastMonth = () => {
    const now = new Date();
    const end = new Date(now.getFullYear(), now.getMonth(), 0);
    return `${end.getFullYear()}-${String(end.getMonth() + 1).padStart(2, "0")}-${String(end.getDate()).padStart(2, "0")}`;
  };
  let to = $state(lastMonth());
  let period = $state({ from: `${app.year}-01-01`, to: `${app.year}-12-31` });

  $effect(() => {
    const [books, year] = [app.books, app.year];
    void app.version;
    if (!books) return;
    period = { from: `${year}-01-01`, to: `${year}-12-31` };
    api.GET("/api/v1/books/{books}/overview", { params: { path: { books }, query: { year } } }).then(({ data }) => (closed = data?.closed_until ?? null));
  });

  async function close(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books) return;
    const { error, response } = await api.POST("/api/v1/books/{books}/closings", { params: { path: { books: app.books } }, body: { to } });
    if (response.ok) message = `${t("closed")}: ${date(to, app.lang)}`;
    else failure = problem(error, response);
  }

  async function exportZip(event: SubmitEvent) {
    event.preventDefault();
    const query = new URLSearchParams(period);
    failure = await download(`/api/v1/books/${app.books}/gobd?${query}`, `gobd-${period.from}-${period.to}.zip`);
  }
</script>

<section class="wrap">
  <h1>{t("closing")}</h1>
  <p>{t("closedUntil")}: {closed ? date(closed, app.lang) : t("nothingClosed")}</p>
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}
  <form class="panel" onsubmit={close}>
    <label>{t("closeUpTo")} <input type="date" bind:value={to} required /></label>
    <div class="row-actions"><button type="submit">{t("close")}</button></div>
  </form>
  <h2>{t("gobd")}</h2>
  <form class="panel" onsubmit={exportZip}>
    <label>{t("from")} <input type="date" bind:value={period.from} required /></label>
    <label>{t("to")} <input type="date" bind:value={period.to} required /></label>
    <div class="row-actions"><button type="submit">{t("exportZip")}</button></div>
  </form>
</section>
