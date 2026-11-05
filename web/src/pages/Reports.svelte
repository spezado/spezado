<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- The reports of the year – the advance VAT returns with their deadlines, the annual ones – their figures, and the
     filings prepared from them. -->
<script lang="ts">
  import { api, problem, save, type Schemas } from "../lib/api";
  import { amount, date } from "../lib/format";
  import Filings from "../lib/Filings.svelte";
  import { app, t } from "../lib/state.svelte";

  /** The reports filed on their own; the API refuses the others. */
  const FILED = ["de-ustva", "de-ust", "de-zm", "de-euer"];

  let reports = $state<Schemas["ReportItem"][]>([]);
  let figures = $state<(Schemas["Figures"] & { from: string; to: string }) | null>(null);
  let failure = $state<string | null>(null);

  $effect(() => {
    const [books, year] = [app.books, app.year];
    void app.version;
    if (!books) return;
    figures = null;
    api.GET("/api/v1/books/{books}/reports", { params: { path: { books }, query: { year } } }).then(({ data }) => (reports = data ?? []));
  });

  async function open(item: Schemas["ReportItem"]) {
    if (!app.books) return;
    const { data, error, response } = await api.GET("/api/v1/books/{books}/reports/{report}", {
      params: { path: { books: app.books, report: item.id }, query: { from: item.from, to: item.to, lang: app.lang } },
    });
    if (data) figures = { ...data, from: item.from, to: item.to };
    else failure = problem(error, response);
  }

  async function prepare() {
    if (!app.books || !figures) return;
    failure = null;
    const result = await api.POST("/api/v1/books/{books}/filings", {
      params: { path: { books: app.books }, query: { lang: app.lang } },
      body: { report: figures.id, from: figures.from, to: figures.to },
    });
    if (!result.data) return void (failure = problem(result.error, result.response));
    figures = null;
    app.version++;
  }

  function csv() {
    if (!figures) return;
    const quote = (text: string) => `"${text.replaceAll('"', '""')}"`;
    const lines = ["field;name;amount", ...figures.fields.map((f) => `${f.field};${quote(f.name ?? "")};${f.amount.value}`)];
    save(new Blob([lines.join("\n") + "\n"], { type: "text/csv" }), `${figures.id}-${figures.from}-${figures.to}.csv`);
  }
</script>

<section class="wrap">
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
  {#if figures}
    <h1>{figures.name}</h1>
    <p>{date(figures.from, app.lang)} – {date(figures.to, app.lang)}</p>
    <p class="error">{t("draft")} · <span class="muted">{figures.law}</span></p>
    <div class="row-actions">
      <button class="quiet" onclick={() => (figures = null)}>{t("back")}</button>
      <button class="quiet" onclick={csv}>{t("csv")}</button>
      {#if FILED.includes(figures.id)}<button onclick={prepare}>{t("prepareFiling")}</button>{/if}
    </div>
    <table>
      <thead><tr><th>{t("field")}</th><th></th><th class="number">{t("amount")}</th></tr></thead>
      <tbody>
        {#each figures.fields as field (field.field)}
          <tr><td class="mono">{field.field}</td><td>{field.name ?? ""}</td><td class="number">{amount(field.amount.value, app.lang)}</td></tr>
          <!-- The remaining terms a balance sheet notes (§ 268 Abs. 4 and 5 HGB). -->
          {@const terms = (figures.terms ?? []).find((t) => t.field === field.field)}
          {#if terms && (Number(terms.over_one_year.value) !== 0 || Number(terms.unknown.value) !== 0)}
            <tr class="muted"><td></td><td>{t("withinAYear")}</td><td class="number">{amount(terms.up_to_one_year.value, app.lang)}</td></tr>
            <tr class="muted"><td></td><td>{t("afterAYear")}</td><td class="number">{amount(terms.over_one_year.value, app.lang)}</td></tr>
            {#if Number(terms.unknown.value) !== 0}
              <tr class="muted"><td></td><td>{t("noTerm")}</td><td class="number">{amount(terms.unknown.value, app.lang)}</td></tr>
            {/if}
          {/if}
        {/each}
      </tbody>
    </table>
    {#if (figures.reconciliation ?? []).length > 0}
      <h2>{t("reconciliation")}</h2>
      <table>
        <tbody>
          {#each figures.reconciliation ?? [] as field (field.field)}
            <tr><td class="mono">{field.field}</td><td>{field.name ?? ""}</td><td class="number">{amount(field.amount.value, app.lang)}</td></tr>
          {/each}
        </tbody>
      </table>
    {/if}
  {:else}
    <h1>{t("reports")} {app.year}</h1>
    {#if reports.length === 0}
      <p class="muted">{t("noReports")}</p>
    {:else}
      <ul class="reports">
        {#each reports as item (`${item.id}/${item.from}`)}
          <li>
            <button class="quiet" onclick={() => open(item)}>{item.name}</button>
            {date(item.from, app.lang)} – {date(item.to, app.lang)}
            {#if item.due}<span class="muted"> · {t("due")} {date(item.due, app.lang)}</span>{/if}
          </li>
        {/each}
      </ul>
    {/if}
    <Filings />
  {/if}
</section>

<style>
  .reports { list-style: none; padding: 0; display: grid; gap: 0.5rem; }
</style>
