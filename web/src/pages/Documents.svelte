<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Documents: a file read in – an e-invoice with its booking proposal, anything else filed to book by hand –, and
     the documents recorded, each with its original. -->
<script lang="ts">
  import { api, download, problem, upload, type Schemas } from "../lib/api";
  import { amount, date, today } from "../lib/format";
  import { app, go, t } from "../lib/state.svelte";

  let documents = $state<Schemas["DocumentView"][]>([]);
  let files = $state<FileList | null>(null);
  let side = $state("purchase");
  let payment = $state("bank");
  let read = $state<Schemas["ReadDocument"] | null>(null);
  let booking = $state({ date: today(), text: "" });
  let failure = $state<string | null>(null);
  let message = $state<string | null>(null);

  $effect(() => {
    const books = app.books;
    void app.version;
    if (!books) return;
    api.GET("/api/v1/books/{books}/documents", { params: { path: { books } } }).then(({ data }) => (documents = data ?? []));
  });

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const file = files?.[0];
    if (!app.books || !file) return;
    failure = message = null;
    const query = new URLSearchParams({ name: file.name, side, payment });
    const result = await upload<Schemas["ReadDocument"]>(`/api/v1/books/${app.books}/documents?${query}`, file);
    read = result.data;
    failure = result.failure;
    if (read) booking = { date: today(), text: read.proposal?.text ?? read.document.name ?? "" };
  }

  async function record(book: boolean) {
    if (!app.books || !read) return;
    const { error, response } = await api.POST("/api/v1/books/{books}/documents/pending/{pending}", {
      params: { path: { books: app.books, pending: read.pending } },
      body: { book, date: booking.date, text: booking.text },
    });
    if (!response.ok) return void (failure = problem(error, response));
    message = book ? t("booked") : t("filed");
    read = null;
  }
</script>

<section class="wrap">
  <h1>{t("documents")}</h1>
  <form class="panel" onsubmit={submit}>
    <label>{t("readDocument")} <span class="muted">{t("documentHint")}</span>
      <input type="file" bind:files accept=".xml,.pdf,image/*" required />
    </label>
    <label>{t("direction")}
      <select bind:value={side}><option value="purchase">{t("purchase")}</option><option value="sale">{t("sale")}</option></select>
    </label>
    <label>{t("payment")}
      <select bind:value={payment}><option value="bank">{t("bank")}</option><option value="cash">{t("cash")}</option></select>
    </label>
    <div class="row-actions"><button type="submit">{t("read")}</button></div>
  </form>
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}

  {#if read}
    <div class="panel read">
      <h2>{read.document.number ?? read.document.name ?? read.document.id}</h2>
      {#if read.not_an_invoice}<p class="error">{t("notAnInvoice")}: {read.not_an_invoice}</p>{/if}
      {#if read.proposal}
        <label>{t("date")} <input type="date" bind:value={booking.date} /></label>
        <label>{t("text")} <input bind:value={booking.text} /></label>
        <table>
          <tbody>
            {#each read.proposal.postings as posting, i (i)}
              <tr>
                <td class="mono">{posting.number ?? ""}</td><td>{posting.name}</td>
                <td class="number">{amount(posting.amount.value, app.lang)}</td><td class="mono muted">{posting.tax ?? ""}</td>
              </tr>
            {/each}
          </tbody>
        </table>
        <div class="row-actions">
          <button onclick={() => record(true)}>{t("fileAndBook")}</button>
          <button class="quiet" onclick={() => record(false)}>{t("fileOnly")}</button>
        </div>
      {:else}
        <p class="muted">{t("noProposal")}</p>
        <div class="row-actions">
          <button onclick={() => record(false)}>{t("fileOnly")}</button>
          <button class="quiet" onclick={() => go("book")}>{t("book")}</button>
        </div>
      {/if}
    </div>
  {/if}

  {#if documents.length === 0}
    <p class="muted">{t("noDocuments")}</p>
  {:else}
    <table class="wide">
      <thead><tr><th>{t("date")}</th><th>{t("number")}</th><th class="number">{t("amount")}</th><th>{t("kind")}</th><th>{t("file")}</th></tr></thead>
      <tbody>
        {#each documents as document (document.id)}
          <tr>
            <td>{date(document.date, app.lang)}</td>
            <td>{document.number ?? ""}</td>
            <td class="number">{document.due ? amount(document.due.value, app.lang) : ""}</td>
            <td class="muted">{document.format ?? document.media_type}{document.booked ? "" : " ·"}</td>
            <td>
              <button class="quiet" onclick={async () => (failure = await download(`/api/v1/books/${app.books}/documents/${document.id}/original`, document.name ?? document.id))}>
                {document.name ?? t("original")}
              </button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>

<style>
  form.panel, .read { margin-bottom: 1rem; display: grid; gap: 0.8rem; }
</style>
