<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
  import { api, problem, type Schemas } from "../lib/api";
  import Deadlines from "../lib/Deadlines.svelte";
  import { amount, date } from "../lib/format";
  import { app, t } from "../lib/state.svelte";

  let overview = $state<Schemas["Overview"] | null>(null);
  let failure = $state<string | null>(null);

  $effect(() => {
    const [books, year] = [app.books, app.year];
    void app.version;
    if (!books) return;
    api.GET("/api/v1/books/{books}/overview", { params: { path: { books }, query: { year } } }).then(({ data, error, response }) => {
      overview = data ?? null;
      failure = data ? null : problem(error, response);
    });
  });

  const money = (value: Schemas["Amount"] | null | undefined) => (value ? amount(value.value, app.lang) : "–");
  const result = $derived.by(() => {
    if (!overview?.income || !overview.expenses) return null;
    const cents = Math.round(Number(overview.income.value) * 100) - Math.round(Number(overview.expenses.value) * 100);
    return (cents / 100).toFixed(2);
  });
</script>

<section class="wrap">
  <h1>{t("overview")}</h1>
  <p class="preview" role="note">{t("preview")}</p>
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
  {#if overview}
    <div class="grid">
      <div class="panel"><div class="muted">{t("bank")}</div><div class="figure">{money(overview.bank)}</div></div>
      <div class="panel"><div class="muted">{t("cash")}</div><div class="figure">{money(overview.cash)}</div></div>
      <div class="panel"><div class="muted">{t("income")} {overview.year}</div><div class="figure">{money(overview.income)}</div></div>
      <div class="panel"><div class="muted">{t("expenses")} {overview.year}</div><div class="figure">{money(overview.expenses)}</div></div>
      <div class="panel"><div class="muted">{t("result")} {overview.year}</div><div class="figure">{result ? amount(result, app.lang) : "–"}</div></div>
    </div>
    <h2>{t("nextVatReturn")}</h2>
    {#if overview.next_vat_return}
      <p>
        {date(overview.next_vat_return.from, app.lang)} – {date(overview.next_vat_return.to, app.lang)},
        {t("due")} <strong>{date(overview.next_vat_return.due, app.lang)}</strong>
      </p>
    {:else}
      <p class="muted">{t("noVatReturns")}</p>
    {/if}
    <Deadlines />
    <p>{t("openEntries")}: {overview.open_entries} · {t("issues")}: {overview.issues} · {t("closedUntil")}:
      {overview.closed_until ? date(overview.closed_until, app.lang) : t("nothingClosed")}</p>
  {/if}
</section>

<style>
  .preview { border-left: 3px solid var(--accent); padding: 0.4rem 0.75rem; color: var(--muted); }
</style>
