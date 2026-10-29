<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- The filings of the books – prepared from a report, submitted through the adapter of their country, asked about –
     and the adapter itself. -->
<script lang="ts">
  import { api, problem, type Schemas } from "./api";
  import { amount, date } from "./format";
  import { app, t } from "./state.svelte";

  const COUNTRY = "DE";

  let filings = $state<Schemas["FilingView"][]>([]);
  let adapter = $state<Schemas["AdapterView"] | null>(null);
  let form = $state<{ url: string; token: string } | null>(null);
  let open = $state<string | null>(null);
  let answer = $state<{ filing: string; text: string } | null>(null);
  let failure = $state<string | null>(null);
  let busy = $state(false);

  $effect(() => {
    const [books, lang] = [app.books, app.lang];
    void app.version;
    if (!books) return;
    api.GET("/api/v1/books/{books}/filings", { params: { path: { books }, query: { lang } } }).then(({ data }) => (filings = data ?? []));
    api.GET("/api/v1/adapters/{country}", { params: { path: { country: COUNTRY } } }).then(({ data }) => (adapter = data ?? null));
  });

  const states: Record<string, () => string> = {
    prepared: () => t("filingPrepared"),
    submitted: () => t("filingSubmitted"),
    accepted: () => t("filingAccepted"),
    rejected: () => t("filingRejected"),
  };

  async function setAdapter(event: SubmitEvent) {
    event.preventDefault();
    if (!form) return;
    failure = null;
    const result = await api.PUT("/api/v1/adapters/{country}", { params: { path: { country: COUNTRY } }, body: form });
    if (!result.data) return void (failure = problem(result.error, result.response));
    adapter = result.data;
    form = null;
  }

  async function submit(filing: Schemas["FilingView"], test: boolean) {
    if (!app.books) return;
    if (!test && !confirm(t("submitConfirm"))) return;
    [busy, failure, answer] = [true, null, null];
    const result = await api.POST("/api/v1/books/{books}/filings/{filing}/submit", {
      params: { path: { books: app.books, filing: filing.id } },
      body: { test },
    });
    busy = false;
    if (!result.data) return void (failure = problem(result.error, result.response));
    answer = { filing: filing.id, text: describe(result.data) };
    if (!test) app.version++;
  }

  async function refresh(filing: Schemas["FilingView"]) {
    if (!app.books) return;
    [busy, failure, answer] = [true, null, null];
    const result = await api.POST("/api/v1/books/{books}/filings/{filing}/refresh", {
      params: { path: { books: app.books, filing: filing.id } },
    });
    busy = false;
    if (!result.data) return void (failure = problem(result.error, result.response));
    answer = { filing: filing.id, text: describe(result.data) };
    app.version++;
  }

  function describe(status: Schemas["SubmissionView"]): string {
    const parts = [(status.test ? `${t("test")}: ` : "") + (states[status.state]?.() ?? status.state)];
    if (status.message) parts.push(status.message);
    if (status.receipts.length > 0) parts.push(`${t("received")}: ${status.receipts.join(", ")}`);
    return parts.join(" · ");
  }
</script>

<h2>{t("filings")}</h2>
<p class="muted">
  {t("adapter")}: {adapter?.url ?? t("noAdapter")}
  {#if !form}<button class="quiet" onclick={() => (form = { url: adapter?.url ?? "", token: "" })}>{t("change")}</button>{/if}
</p>
{#if form}
  <form class="card" onsubmit={setAdapter}>
    <p class="muted">{t("adapterText")}</p>
    <label>{t("address")} <input type="url" bind:value={form.url} placeholder="https://" required /></label>
    <label>{t("token")} <input type="password" bind:value={form.token} autocomplete="off" required /></label>
    <div class="row-actions">
      <button type="button" class="quiet" onclick={() => (form = null)}>{t("cancel")}</button>
      <button type="submit">{t("apply")}</button>
    </div>
  </form>
{/if}
{#if failure}<p class="error" role="alert">{failure}</p>{/if}
{#if filings.length === 0}
  <p class="muted">{t("noFilings")}</p>
{:else}
  <ul class="filings">
    {#each filings as filing (filing.id)}
      <li class="card">
        <button class="quiet" onclick={() => (open = open === filing.id ? null : filing.id)}>
          {filing.report} · {date(filing.from, app.lang)} – {date(filing.to, app.lang)}
        </button>
        <span>{states[filing.state]?.() ?? filing.state}</span>
        {#if filing.corrects}<span class="muted"> · {t("correction")}</span>{/if}
        {#if filing.outdated}<span class="error"> · {t("outdated")}</span>{/if}
        {#if filing.documents.length > 0}<span class="muted"> · {t("documents")}: {filing.documents.length}</span>{/if}
        {#if open === filing.id}
          <table>
            <tbody>
              {#each filing.fields as field (field.field)}
                <tr><td class="mono">{field.field}</td><td>{field.name ?? ""}</td><td class="number">{amount(field.amount.value, app.lang)}</td></tr>
              {/each}
            </tbody>
          </table>
        {/if}
        <div class="row-actions">
          {#if filing.state === "prepared" || filing.state === "rejected"}
            <button class="quiet" disabled={busy || !adapter?.url} onclick={() => submit(filing, true)}>{t("submitTest")}</button>
            <button disabled={busy || !adapter?.url || filing.outdated} onclick={() => submit(filing, false)}>{t("submit")}</button>
          {:else if filing.state === "submitted"}
            <button class="quiet" disabled={busy} onclick={() => refresh(filing)}>{t("askState")}</button>
          {/if}
        </div>
        {#if answer?.filing === filing.id}<p role="status">{answer.text}</p>{/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  .filings { list-style: none; padding: 0; display: grid; gap: 0.5rem; }
</style>
