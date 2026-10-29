<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
  import { api, problem, type Schemas } from "../lib/api";
  import { amount, date, today } from "../lib/format";
  import { app, t } from "../lib/state.svelte";

  let entries = $state<Schemas["JournalEntry"][]>([]);
  let selected = $state(0);
  let failure = $state<string | null>(null);
  let reversing = $state<{ date: string; text: string } | null>(null);

  $effect(() => {
    const [books, year, lang] = [app.books, app.year, app.lang];
    void app.version;
    if (!books) return;
    api.GET("/api/v1/books/{books}/journal", { params: { path: { books }, query: { year, lang } } }).then(({ data, error, response }) => {
      entries = data ?? [];
      failure = data ? null : problem(error, response);
      selected = Math.min(selected, Math.max(entries.length - 1, 0));
    });
  });

  const entry = $derived(entries[selected]);

  async function reverse(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books || !entry || !reversing) return;
    const { error, response } = await api.POST("/api/v1/books/{books}/reversals", {
      params: { path: { books: app.books } },
      body: { event: entry.event, date: reversing.date, text: reversing.text },
    });
    if (response.ok) reversing = null;
    else failure = problem(error, response);
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown") selected = Math.min(selected + 1, entries.length - 1);
    else if (event.key === "ArrowUp") selected = Math.max(selected - 1, 0);
    else return;
    event.preventDefault();
  }
</script>

<section class="wrap">
  <h1>{t("journal")} {app.year}</h1>
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
  {#if entries.length === 0}
    <p class="muted">{t("nothingBooked")}</p>
  {:else}
    <table class="wide" role="grid" tabindex="0" onkeydown={keydown} aria-label={t("journal")}>
      <thead><tr><th>{t("date")}</th><th>{t("text")}</th><th class="number">{t("amount")}</th><th></th></tr></thead>
      <tbody>
        {#each entries as item, i (item.event)}
          <tr class="row" class:selected={i === selected} onclick={() => (selected = i)} aria-selected={i === selected}>
            <td>{date(item.date, app.lang)}</td>
            <td>{item.text}</td>
            <td class="number">{amount(item.amount.value, app.lang)}</td>
            <td class="muted">{item.reverses ? t("reversal") : item.reversed_by ? t("reversed") : ""}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    {#if entry}
      <div class="panel detail">
        <table>
          <tbody>
            {#each entry.postings as posting, i (i)}
              <tr>
                <td class="mono">{posting.number ?? ""}</td>
                <td>{posting.name}</td>
                <td class="number">{amount(posting.amount.value, app.lang)}</td>
                <td class="muted mono">{posting.tax ?? ""}</td>
              </tr>
            {/each}
          </tbody>
        </table>
        {#if !entry.reverses && !entry.reversed_by}
          {#if reversing}
            <form onsubmit={reverse}>
              <label>{t("date")} <input type="date" bind:value={reversing.date} required /></label>
              <label>{t("reason")} <input bind:value={reversing.text} required /></label>
              <div class="row-actions">
                <button type="submit">{t("reverse")}</button>
                <button type="button" class="quiet" onclick={() => (reversing = null)}>{t("cancel")}</button>
              </div>
            </form>
          {:else}
            <div class="row-actions"><button class="quiet" onclick={() => (reversing = { date: today(), text: "" })}>{t("reverse")}</button></div>
          {/if}
        {/if}
      </div>
    {/if}
  {/if}
</section>

<style>
  .detail { margin-top: 1rem; display: grid; gap: 1rem; }
</style>
