<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
  import AccountPicker from "../lib/AccountPicker.svelte";
  import { api, problem, type Schemas } from "../lib/api";
  import { parseAmount, today } from "../lib/format";
  import { app, go, t } from "../lib/state.svelte";

  let accounts = $state<Schemas["AccountView"][]>([]);
  let codes = $state<Schemas["TaxCodeView"][]>([]);
  let mode = $state<"simple" | "lines">("simple");
  let failure = $state<string | null>(null);
  let message = $state<string | null>(null);

  let simple = $state({ date: today(), text: "", account: "", gross: "", tax: "", payment: "bank", taxTouched: false });
  let lines = $state({ date: today(), text: "", rows: [{ account: "", amount: "", tax: "" }, { account: "", amount: "", tax: "" }] });

  $effect(() => {
    const [books, lang] = [app.books, app.lang];
    if (!books) return;
    api.GET("/api/v1/books/{books}/accounts", { params: { path: { books }, query: { lang } } }).then(({ data }) => (accounts = data ?? []));
    api.GET("/api/v1/books/{books}/tax-codes", { params: { path: { books }, query: { lang } } }).then(({ data }) => (codes = data ?? []));
  });

  /** A tax code nobody chose follows the kind of the account chosen. */
  function chosen(account: Schemas["AccountView"]) {
    if (simple.taxTouched) return;
    const code = account.kind === "income" ? "de-out-19" : account.kind === "expense" ? "de-in-19" : "";
    simple.tax = codes.some((c) => c.code === code) ? code : "";
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books) return;
    failure = message = null;
    const books = app.books;
    let result;
    if (mode === "simple") {
      const gross = parseAmount(simple.gross, app.lang);
      if (gross === null) return void (failure = `${t("gross")}?`);
      result = await api.POST("/api/v1/books/{books}/bookings/simple", {
        params: { path: { books } },
        body: { date: simple.date, text: simple.text, account: simple.account, gross, tax: simple.tax || null, payment: simple.payment },
      });
    } else {
      const rows = lines.rows.map((row) => ({ ...row, amount: parseAmount(row.amount, app.lang) }));
      if (rows.some((row) => row.amount === null)) return void (failure = `${t("amount")}?`);
      result = await api.POST("/api/v1/books/{books}/bookings/lines", {
        params: { path: { books } },
        body: { date: lines.date, text: lines.text, lines: rows.map((row) => ({ account: row.account, amount: row.amount!, tax: row.tax || null })) },
      });
    }
    if (!result.response.ok) return void (failure = problem(result.error, result.response));
    message = t("booked");
    simple = { ...simple, text: "", account: "", gross: "", taxTouched: false };
    lines = { date: lines.date, text: "", rows: [{ account: "", amount: "", tax: "" }, { account: "", amount: "", tax: "" }] };
  }
</script>

<section class="wrap">
  <h1>{t("book")}</h1>
  <div class="row-actions modes" role="tablist">
    <button role="tab" aria-selected={mode === "simple"} class:quiet={mode !== "simple"} onclick={() => (mode = "simple")}>{t("simple")}</button>
    <button role="tab" aria-selected={mode === "lines"} class:quiet={mode !== "lines"} onclick={() => (mode = "lines")}>{t("lines")}</button>
  </div>
  <form onsubmit={submit}>
    {#if mode === "simple"}
      <label>{t("date")} <input type="date" bind:value={simple.date} required /></label>
      <label>{t("text")} <input bind:value={simple.text} required /></label>
      <AccountPicker {accounts} bind:value={simple.account} label={t("account")} onchosen={chosen} />
      <label>{t("gross")} <input inputmode="decimal" bind:value={simple.gross} required /></label>
      <label>{t("taxCode")}
        <select bind:value={simple.tax} onchange={() => (simple.taxTouched = true)}>
          <option value="">{t("noTax")}</option>
          {#each codes as code (code.code)}<option value={code.code}>{code.code} – {code.name}</option>{/each}
        </select>
      </label>
      <label>{t("payment")}
        <select bind:value={simple.payment}>
          <option value="bank">{t("bank")}</option>
          <option value="cash">{t("cash")}</option>
        </select>
      </label>
    {:else}
      <label>{t("date")} <input type="date" bind:value={lines.date} required /></label>
      <label>{t("text")} <input bind:value={lines.text} required /></label>
      {#each lines.rows as row, i (i)}
        <fieldset>
          <AccountPicker {accounts} bind:value={row.account} label={`${t("account")} ${i + 1}`} />
          <label>{t("amount")} <input inputmode="decimal" bind:value={row.amount} required /></label>
          <label>{t("taxCode")}
            <select bind:value={row.tax}>
              <option value="">{t("noTax")}</option>
              {#each codes as code (code.code)}<option value={code.code}>{code.code}</option>{/each}
            </select>
          </label>
          {#if lines.rows.length > 2}
            <button type="button" class="quiet" onclick={() => lines.rows.splice(i, 1)}>{t("remove")}</button>
          {/if}
        </fieldset>
      {/each}
      <div class="row-actions">
        <button type="button" class="quiet" onclick={() => lines.rows.push({ account: "", amount: "", tax: "" })}>{t("addLine")}</button>
      </div>
    {/if}
    {#if failure}<p class="error" role="alert">{failure}</p>{/if}
    {#if message}<p role="status">{message} · <a href="#/journal" onclick={() => go("journal")}>{t("journal")}</a></p>{/if}
    <div class="row-actions"><button type="submit">{t("save")}</button></div>
  </form>
</section>

<style>
  .modes { margin-bottom: 1rem; }
  fieldset { border: 1px solid var(--line); border-radius: 6px; display: grid; gap: 0.6rem; padding: 0.75rem; }
</style>
