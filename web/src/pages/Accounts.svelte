<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- The accounts with a balance at the end of the year, and the ledger of each: its bookings with a running balance. -->
<script lang="ts">
  import Opening from "../lib/Opening.svelte";
  import { api, type Schemas } from "../lib/api";
  import { amount, date } from "../lib/format";
  import { app, t } from "../lib/state.svelte";

  let balances = $state<Schemas["BalanceView"][]>([]);
  let journal = $state<Schemas["JournalEntry"][]>([]);
  let account = $state<Schemas["BalanceView"] | null>(null);

  $effect(() => {
    const [books, year, lang] = [app.books, app.year, app.lang];
    void app.version;
    if (!books) return;
    api.GET("/api/v1/books/{books}/balances", { params: { path: { books }, query: { year, lang } } }).then(({ data }) => (balances = data ?? []));
    api.GET("/api/v1/books/{books}/journal", { params: { path: { books }, query: { year, lang } } }).then(({ data }) => (journal = data ?? []));
  });

  /** The bookings of the account chosen, in cents, with the balance after each. */
  const ledger = $derived.by(() => {
    if (!account) return [];
    let running = 0;
    return journal.flatMap((entry) =>
      entry.postings
        .filter((p) => p.account === account!.account)
        .map((p) => {
          const cents = Math.round(Number(p.amount.value) * 100);
          running += cents;
          return { date: entry.date, text: entry.text, amount: p.amount.value, balance: (running / 100).toFixed(2) };
        }),
    );
  });
</script>

<section class="wrap">
  {#if account}
    <h1>{t("ledger")}: {account.number ?? ""} {account.name}</h1>
    <div class="row-actions"><button class="quiet" onclick={() => (account = null)}>{t("back")}</button></div>
    <table class="wide">
      <thead><tr><th>{t("date")}</th><th>{t("text")}</th><th class="number">{t("amount")}</th><th class="number">{t("balance")}</th></tr></thead>
      <tbody>
        {#each ledger as row, i (i)}
          <tr><td>{date(row.date, app.lang)}</td><td>{row.text}</td><td class="number">{amount(row.amount, app.lang)}</td><td class="number">{amount(row.balance, app.lang)}</td></tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <h1>{t("accounts")} {app.year}</h1>
    <table>
      <thead><tr><th>{t("number")}</th><th>{t("account")}</th><th class="number">{t("balance")}</th></tr></thead>
      <tbody>
        {#each balances as row (row.account)}
          <tr class="row" onclick={() => (account = row)}>
            <td class="mono">{row.number ?? ""}</td>
            <td><button class="link" onclick={() => (account = row)}>{row.name}</button></td>
            <td class="number">{amount(row.balance.value, app.lang)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    <Opening />
  {/if}
</section>

<style>
  .link { background: none; border: none; color: var(--text); padding: 0; text-align: left; }
  .link:hover { color: var(--accent); }
</style>
