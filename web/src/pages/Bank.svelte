<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- The bank: statements read from files, the entries not booked yet – by their proposal, or on an account by hand. -->
<script lang="ts">
  import AccountPicker from "../lib/AccountPicker.svelte";
  import { api, problem, upload, type Schemas } from "../lib/api";
  import { amount, date } from "../lib/format";
  import { app, t } from "../lib/state.svelte";

  let entries = $state<Schemas["OpenEntry"][]>([]);
  let accounts = $state<Schemas["AccountView"][]>([]);
  let codes = $state<Schemas["TaxCodeView"][]>([]);
  let failure = $state<string | null>(null);
  let message = $state<string | null>(null);
  let files = $state<FileList | null>(null);
  let iban = $state("");
  let open = $state<string | null>(null);
  let entry = $state({ account: "", tax: "", text: "" });

  const key = (e: Schemas["OpenEntry"]) => `${e.document}/${e.entry}`;

  $effect(() => {
    const [books, lang] = [app.books, app.lang];
    void app.version;
    if (!books) return;
    api.GET("/api/v1/books/{books}/bank/entries", { params: { path: { books } } }).then(({ data, error, response }) => {
      entries = data ?? [];
      if (!data) failure = problem(error, response);
    });
    api.GET("/api/v1/books/{books}/accounts", { params: { path: { books }, query: { lang } } }).then(({ data }) => (accounts = data ?? []));
    api.GET("/api/v1/books/{books}/tax-codes", { params: { path: { books }, query: { lang } } }).then(({ data }) => (codes = data ?? []));
  });

  async function read(event: SubmitEvent) {
    event.preventDefault();
    const file = files?.[0];
    if (!app.books || !file) return;
    failure = message = null;
    const query = new URLSearchParams({ name: file.name });
    if (iban.trim()) query.set("iban", iban.trim());
    const { data, failure: wrong } = await upload<Schemas["Imported"]>(`/api/v1/books/${app.books}/bank/statements?${query}`, file);
    if (!data) return void (failure = wrong);
    message = `${t("newEntries")}: ${data.entries} · ${t("knownBefore")}: ${data.shown_before + data.known}`;
  }

  async function accept(item: Schemas["OpenEntry"]) {
    if (!app.books) return;
    const { error, response } = await api.POST("/api/v1/books/{books}/bank/entries/accept", {
      params: { path: { books: app.books } },
      body: { document: item.document, entry: item.entry },
    });
    failure = response.ok ? null : problem(error, response);
  }

  async function acceptAll() {
    if (!app.books) return;
    const { data, error, response } = await api.POST("/api/v1/books/{books}/bank/accept-all", { params: { path: { books: app.books } } });
    if (data) message = `${t("accepted")}: ${data.count}`;
    else failure = problem(error, response);
  }

  async function fetchFromBank() {
    if (!app.books) return;
    const { error, response } = await api.POST("/api/v1/books/{books}/bank/fetch", { params: { path: { books: app.books } } });
    failure = response.ok ? null : problem(error, response);
  }

  function expand(item: Schemas["OpenEntry"]) {
    open = open === key(item) ? null : key(item);
    entry = { account: "", tax: "", text: item.text };
  }

  function chosen(account: Schemas["AccountView"]) {
    const code = account.kind === "income" ? "de-out-19" : account.kind === "expense" ? "de-in-19" : "";
    entry.tax = codes.some((c) => c.code === code) ? code : "";
  }

  async function book(event: SubmitEvent, item: Schemas["OpenEntry"]) {
    event.preventDefault();
    if (!app.books) return;
    const { error, response } = await api.POST("/api/v1/books/{books}/bank/entries/book", {
      params: { path: { books: app.books } },
      body: { document: item.document, entry: item.entry, account: entry.account, tax: entry.tax || null, text: entry.text },
    });
    if (response.ok) open = null;
    else failure = problem(error, response);
  }

  function label(item: Schemas["OpenEntry"]): string {
    const s = item.suggestion;
    if (!s) return "";
    if (s.source === "invoice") return `${t("fromInvoice")} ${s.number ?? ""}`;
    if (s.source === "booked") return `${t("bookedBefore")}: ${s.booking.text}`;
    // A booking proposed as before has the account it books on first, then VAT, then the bank.
    const counter = s.booking.postings[0];
    return `${t("asBefore")}: ${counter?.number ?? ""} ${counter?.name ?? ""}`;
  }
</script>

<section class="wrap">
  <h1>{t("bank")}</h1>
  <form class="panel" onsubmit={read}>
    <label>{t("readStatement")} <span class="muted">{t("statementHint")}</span>
      <input type="file" bind:files accept=".xml,.sta,.mt940,.txt,.csv" required />
    </label>
    <label>{t("ibanForCsv")} <input bind:value={iban} autocomplete="off" /></label>
    <div class="row-actions">
      <button type="submit">{t("read")}</button>
      <button type="button" class="quiet" onclick={fetchFromBank}>{t("fetch")}</button>
    </div>
  </form>
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}

  <h2>{t("openEntries")}</h2>
  {#if entries.length === 0}
    <p class="muted">{t("noOpenEntries")}</p>
  {:else}
    {#if entries.some((e) => e.suggestion)}
      <div class="row-actions"><button onclick={acceptAll}>{t("acceptAll")}</button></div>
    {/if}
    <table class="wide">
      <thead><tr><th>{t("date")}</th><th>{t("text")}</th><th class="number">{t("amount")}</th><th>{t("proposal")}</th><th></th></tr></thead>
      <tbody>
        {#each entries as item (key(item))}
          <tr>
            <td>{date(item.date, app.lang)}</td>
            <td>{item.text}</td>
            <td class="number">{amount(item.amount.value, app.lang)}</td>
            <td class="accent">{label(item)}</td>
            <td class="row-actions">
              {#if item.suggestion}<button onclick={() => accept(item)}>{t("accept")}</button>{/if}
              <button class="quiet" onclick={() => expand(item)} aria-expanded={open === key(item)}>{t("bookEntry")}</button>
            </td>
          </tr>
          {#if open === key(item)}
            <tr>
              <td colspan="5">
                <form onsubmit={(event) => book(event, item)}>
                  <label>{t("text")} <input bind:value={entry.text} required /></label>
                  <AccountPicker {accounts} bind:value={entry.account} label={t("account")} onchosen={chosen} />
                  <label>{t("taxCode")}
                    <select bind:value={entry.tax}>
                      <option value="">{t("noTax")}</option>
                      {#each codes as code (code.code)}<option value={code.code}>{code.code} – {code.name}</option>{/each}
                    </select>
                  </label>
                  <div class="row-actions"><button type="submit">{t("bookEntry")}</button></div>
                </form>
              </td>
            </tr>
          {/if}
        {/each}
      </tbody>
    </table>
  {/if}
</section>

<style>
  form.panel { margin-bottom: 1rem; }
  .accent { color: var(--accent); }
</style>
