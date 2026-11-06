<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- What is due next: VAT returns, statements per customer, wage tax returns, statements of contributions,
     certificates of wage tax, the annual returns – from today on, by the day due. -->
<script lang="ts">
  import { api, type Schemas } from "./api";
  import { date, today } from "./format";
  import { app, t } from "./state.svelte";

  let items = $state<Schemas["Obligation"][]>([]);

  $effect(() => {
    const [books, year] = [app.books, app.year];
    void app.version;
    if (!books) return;
    Promise.all([year, year + 1].map((y) => api.GET("/api/v1/books/{books}/obligations", { params: { path: { books }, query: { year: y } } }))).then(
      (results) => {
        const now = today();
        items = results
          .flatMap((r) => r.data ?? [])
          .filter((o) => o.due && o.due >= now)
          .sort((a, b) => (a.due ?? "").localeCompare(b.due ?? ""))
          .slice(0, 8);
      },
    );
  });
</script>

<h2>{t("obligations")}</h2>
{#if items.length === 0}
  <p class="muted">{t("noObligations")}</p>
{:else}
  <table>
    <tbody>
      {#each items as o (o.id + o.from)}
        <tr>
          <td><strong>{o.due ? date(o.due, app.lang) : "–"}</strong></td>
          <td>{o.name}</td>
          <td class="muted">{date(o.from, app.lang)} – {date(o.to, app.lang)}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}
