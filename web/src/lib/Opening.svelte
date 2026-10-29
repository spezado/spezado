<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- The opening balances: what accounts held on the first day of the books, booked against the opening balance sheet
     so that they count in balances and in no result. -->
<script lang="ts">
  import AccountPicker from "./AccountPicker.svelte";
  import { api, problem, type Schemas } from "./api";
  import { amount, parseAmount } from "./format";
  import { app, t } from "./state.svelte";

  let view = $state<Schemas["OpeningView"] | null>(null);
  let accounts = $state<Schemas["AccountView"][]>([]);
  let editing = $state(false);
  let day = $state("");
  let rows = $state([{ account: "", amount: "" }]);
  let failure = $state<string | null>(null);

  $effect(() => {
    const [books, lang] = [app.books, app.lang];
    void app.version;
    if (!books) return;
    api.GET("/api/v1/books/{books}/opening", { params: { path: { books }, query: { lang } } }).then(({ data }) => {
      view = data ?? null;
      if (!day && data?.start) day = data.start;
    });
    api.GET("/api/v1/books/{books}/accounts", { params: { path: { books }, query: { lang } } }).then(({ data }) => (accounts = data ?? []));
  });

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books) return;
    failure = null;
    const lines = rows.filter((row) => row.account).map((row) => ({ account: row.account, amount: parseAmount(row.amount, app.lang) }));
    if (lines.some((line) => line.amount === null)) return void (failure = `${t("amount")}?`);
    const result = await api.POST("/api/v1/books/{books}/opening", {
      params: { path: { books: app.books } },
      body: { date: day || null, text: t("openingBalances"), lines: lines.map((line) => ({ account: line.account, amount: line.amount! })) },
    });
    if (!result.response.ok) return void (failure = problem(result.error, result.response));
    editing = false;
    rows = [{ account: "", amount: "" }];
  }
</script>

<section class="opening">
  <h2>{t("openingBalances")}</h2>
  {#if view && view.balances.length > 0}
    <table>
      <tbody>
        {#each view.balances as row (row.account)}
          <tr><td class="mono">{row.number ?? ""}</td><td>{row.name}</td><td class="number">{amount(row.amount.value, app.lang)}</td></tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p>{t("noOpening")}</p>
  {/if}
  {#if editing}
    <form class="card" onsubmit={submit}>
      <p class="hint">{t("openingHint")}</p>
      <label>{t("date")} <input type="date" bind:value={day} required /></label>
      {#each rows as row, i (i)}
        <fieldset>
          <AccountPicker {accounts} bind:value={row.account} label={`${t("account")} ${i + 1}`} />
          <label>{t("amount")} <input inputmode="decimal" bind:value={row.amount} required /></label>
          {#if rows.length > 1}
            <button type="button" class="quiet" onclick={() => rows.splice(i, 1)}>{t("remove")}</button>
          {/if}
        </fieldset>
      {/each}
      {#if failure}<p class="error" role="alert">{failure}</p>{/if}
      <div class="row-actions">
        <button type="button" class="quiet" onclick={() => rows.push({ account: "", amount: "" })}>{t("addLine")}</button>
        <button type="button" class="quiet" onclick={() => (editing = false)}>{t("cancel")}</button>
        <button type="submit">{t("save")}</button>
      </div>
    </form>
  {:else}
    <div class="row-actions"><button class="quiet" onclick={() => (editing = true)}>{t("enterOpening")}</button></div>
  {/if}
</section>

<style>
  .opening { margin-top: 2rem; }
  fieldset { border: 1px solid var(--line); border-radius: 6px; display: grid; gap: 0.6rem; padding: 0.75rem; }
  .hint { color: var(--muted); }
</style>
