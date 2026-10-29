<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- The reports of the year – the advance VAT returns with their deadlines, the annual ones – and their figures. -->
<script lang="ts">
  import { api, problem, save, type Schemas } from "../lib/api";
  import { amount, date } from "../lib/format";
  import { app, t } from "../lib/state.svelte";

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
    </div>
    <table>
      <thead><tr><th>{t("field")}</th><th></th><th class="number">{t("amount")}</th></tr></thead>
      <tbody>
        {#each figures.fields as field (field.field)}
          <tr><td class="mono">{field.field}</td><td>{field.name ?? ""}</td><td class="number">{amount(field.amount.value, app.lang)}</td></tr>
        {/each}
      </tbody>
    </table>
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
  {/if}
</section>

<style>
  .reports { list-style: none; padding: 0; display: grid; gap: 0.5rem; }
</style>
