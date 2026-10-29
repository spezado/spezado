<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- The frame: pairing first, then setting up books, then the pages – read again whenever the books change. -->
<script lang="ts">
  import { api, onChange, problem, setToken, token, type Schemas } from "./lib/api";
  import { app, go, setLang, setTheme, t, type Page, type Theme } from "./lib/state.svelte";
  import Accounts from "./pages/Accounts.svelte";
  import Bank from "./pages/Bank.svelte";
  import Book from "./pages/Book.svelte";
  import Closing from "./pages/Closing.svelte";
  import Documents from "./pages/Documents.svelte";
  import Invoices from "./pages/Invoices.svelte";
  import Journal from "./pages/Journal.svelte";
  import Overview from "./pages/Overview.svelte";
  import Pair from "./pages/Pair.svelte";
  import Reports from "./pages/Reports.svelte";
  import Review from "./pages/Review.svelte";
  import SetUp from "./pages/SetUp.svelte";

  const code = new URLSearchParams(location.hash.slice(1)).get("pair") ?? "";
  let paired = $state(token() !== null && !code);
  let books = $state<Schemas["BooksSummary"][] | null>(null);
  let failure = $state<string | null>(null);

  async function load() {
    try {
      const { data, error, response } = await api.GET("/api/v1/books");
      if (response.status === 401) {
        setToken(null);
        paired = false;
        return;
      }
      failure = data ? null : problem(error, response);
      books = data ?? null;
      app.books = data?.[0]?.id ?? null;
    } catch {
      failure = t("offline");
    }
  }

  $effect(() => {
    if (!paired) return;
    load();
    return onChange(() => {
      app.version += 1;
      load();
    });
  });

  const pages: Page[] = ["overview", "journal", "bank", "documents", "invoices", "accounts", "reports", "review", "closing"];
  const owner = $derived(books?.[0]?.owner_name ?? "");
</script>

{#if !paired}
  <Pair {code} onpaired={() => (paired = true)} />
{:else if books && books.length === 0}
  <SetUp onsetup={load} />
{:else}
  <header>
    <div class="wrap bar">
      <a class="brand" href="#/overview" aria-label="spezado">
        <img class="on-light" src="/spezado-lockup-on-light.svg" alt="spezado" height="36" />
        <img class="on-dark" src="/spezado-lockup-on-dark.svg" alt="spezado" height="36" />
      </a>
      <span class="owner">{owner}</span>
      <nav aria-label="spezado">
        {#each pages as page (page)}
          <a href={`#/${page}`} aria-current={app.page === page ? "page" : undefined} onclick={() => go(page)}>{t(page)}</a>
        {/each}
      </nav>
      <a class="book" href="#/book" onclick={() => go("book")}>+ {t("book")}</a>
      <div class="tools">
        <label class="year">{t("year")} <input type="number" bind:value={app.year} min="2000" max="2099" /></label>
        <button class="quiet" onclick={() => setLang(app.lang === "de" ? "en" : "de")}>{t("language")}</button>
        <label class="theme">
          <span class="visually-hidden">{t("theme")}</span>
          <select value={app.theme} onchange={(e) => setTheme((e.currentTarget as HTMLSelectElement).value as Theme)} aria-label={t("theme")}>
            <option value="auto">{t("auto")}</option>
            <option value="light">{t("light")}</option>
            <option value="dark">{t("dark")}</option>
          </select>
        </label>
      </div>
    </div>
  </header>
  <main>
    {#if failure}<p class="wrap error" role="alert">{failure}</p>{/if}
    {#if app.page === "journal"}<Journal />
    {:else if app.page === "book"}<Book />
    {:else if app.page === "bank"}<Bank />
    {:else if app.page === "documents"}<Documents />
    {:else if app.page === "invoices"}<Invoices />
    {:else if app.page === "accounts"}<Accounts />
    {:else if app.page === "reports"}<Reports />
    {:else if app.page === "review"}<Review />
    {:else if app.page === "closing"}<Closing />
    {:else}<Overview />{/if}
  </main>
{/if}

<style>
  header { border-bottom: 1px solid var(--line); background: var(--surface); margin-bottom: 1.5rem; }
  .bar { display: flex; align-items: center; gap: 1.5rem; flex-wrap: wrap; padding-block: 0.75rem; }
  .brand { display: flex; }
  .owner { font-family: var(--serif); color: var(--muted); }
  nav { display: flex; gap: 1rem; flex-wrap: wrap; }
  .book { background: var(--accent); color: var(--on-accent); padding: 0.35rem 0.8rem; border-radius: 6px; text-decoration: none; }
  nav a { color: var(--text); text-decoration: none; padding: 0.2rem 0; border-bottom: 2px solid transparent; }
  nav a[aria-current="page"] { border-bottom-color: var(--accent); color: var(--accent); }
  .tools { margin-left: auto; display: flex; gap: 0.75rem; align-items: center; }
  .year { display: flex; flex-direction: row; align-items: center; gap: 0.4rem; }
  .year input { width: 5.5rem; }
  .visually-hidden { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
  main { padding-bottom: 3rem; }
</style>
