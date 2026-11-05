<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Fixed assets: the register with book values, a new asset, a disposal, and the depreciation of the year. -->
<script lang="ts">
  import { api, problem, type Schemas } from "../lib/api";
  import { amount, date, today } from "../lib/format";
  import { app, t } from "../lib/state.svelte";

  type Asset = Schemas["AssetView"];
  let assets = $state<Asset[]>([]);
  let failure = $state<string | null>(null);
  let message = $state<string | null>(null);
  let form = $state<{ name: string; account: string; acquired: string; cost: string; method: string; months: number } | null>(null);
  let leaving = $state<{ asset: Asset; date: string; proceeds: string } | null>(null);

  $effect(() => {
    const [books, year] = [app.books, app.year];
    void app.version;
    if (!books) return;
    const at = `${year}-12-31`;
    api.GET("/api/v1/books/{books}/assets", { params: { path: { books }, query: { at } } }).then(({ data }) => (assets = data ?? []));
  });

  const methodText = (a: Asset) =>
    a.method === "straight_line" ? `${t("straightLine")}, ${a.months} ${t("months")}` : t(({ immediate: "immediate", pool: "pool", none: "notDepreciated" } as const)[a.method as "pool"]);

  async function register(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books || !form) return;
    failure = message = null;
    const body = { ...form, cost: form.cost.replace(",", "."), months: form.method === "straight_line" ? form.months : null };
    const { error, response } = await api.POST("/api/v1/books/{books}/assets", { params: { path: { books: app.books } }, body });
    if (!response.ok) return void (failure = problem(error, response));
    message = `${t("assetRegistered")}: ${form.name}`;
    form = null;
  }

  async function dispose(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books || !leaving) return;
    failure = message = null;
    const body = { date: leaving.date, proceeds: leaving.proceeds ? leaving.proceeds.replace(",", ".") : null };
    const { error, response } = await api.POST("/api/v1/books/{books}/assets/{asset}/disposal", {
      params: { path: { books: app.books, asset: leaving.asset.id } },
      body,
    });
    if (!response.ok) return void (failure = problem(error, response));
    message = `${t("assetLeft")}: ${leaving.asset.name}`;
    leaving = null;
  }

  async function depreciate() {
    if (!app.books) return;
    failure = message = null;
    const { data, error, response } = await api.POST("/api/v1/books/{books}/depreciation", {
      params: { path: { books: app.books } },
      body: { year: app.year },
    });
    if (!response.ok) return void (failure = problem(error, response));
    message = data ? `${t("depreciationBooked")} ${app.year}` : t("nothingToDepreciate");
  }
</script>

<section class="wrap">
  <h1>{t("assets")}</h1>
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}
  <div class="row-actions">
    <button onclick={() => (form = { name: "", account: "0420", acquired: today(), cost: "", method: "straight_line", months: 36 })}>+ {t("newAsset")}</button>
    <button class="quiet" onclick={depreciate}>{t("bookDepreciation")} {app.year}</button>
  </div>

  {#if form}
    <form class="panel" onsubmit={register}>
      <h2>{t("newAsset")}</h2>
      <div class="grid">
        <label>{t("name")} <input bind:value={form.name} required /></label>
        <label>{t("account")} <input bind:value={form.account} required /></label>
        <label>{t("acquired")} <input type="date" bind:value={form.acquired} required /></label>
        <label>{t("costNet")} <input inputmode="decimal" bind:value={form.cost} required /></label>
        <label>{t("method")}
          <select bind:value={form.method}>
            <option value="straight_line">{t("straightLine")}</option>
            <option value="immediate">{t("immediate")}</option>
            <option value="pool">{t("pool")}</option>
            <option value="none">{t("notDepreciated")}</option>
          </select>
        </label>
        {#if form.method === "straight_line"}<label>{t("months")} <input type="number" min="1" bind:value={form.months} required /></label>{/if}
      </div>
      <p class="muted">{t("assetHint")}</p>
      <div class="row-actions">
        <button type="button" class="quiet" onclick={() => (form = null)}>{t("cancel")}</button>
        <button type="submit">{t("keep")}</button>
      </div>
    </form>
  {/if}

  {#if leaving}
    <form class="panel" onsubmit={dispose}>
      <h2>{t("assetLeaves")}: {leaving.asset.name}</h2>
      <label>{t("date")} <input type="date" bind:value={leaving.date} required /></label>
      <label>{t("proceeds")} <input inputmode="decimal" bind:value={leaving.proceeds} /></label>
      <div class="row-actions">
        <button type="button" class="quiet" onclick={() => (leaving = null)}>{t("cancel")}</button>
        <button type="submit">{t("keep")}</button>
      </div>
    </form>
  {/if}

  {#if assets.length === 0}
    <p class="muted">{t("noAssets")}</p>
  {:else}
    <table class="wide">
      <thead><tr><th>{t("name")}</th><th>{t("account")}</th><th>{t("acquired")}</th><th class="number">{t("costNet")}</th><th>{t("method")}</th><th class="number">{t("bookValue")}</th><th></th></tr></thead>
      <tbody>
        {#each assets as asset (asset.id)}
          <tr>
            <td>{asset.name}</td>
            <td class="mono">{asset.account}</td>
            <td>{date(asset.acquired, app.lang)}</td>
            <td class="number">{amount(asset.cost.value, app.lang)}</td>
            <td>{methodText(asset)}</td>
            <td class="number">{amount(asset.book_value.value, app.lang)}</td>
            <td>
              {#if asset.disposed}{t("left")} {date(asset.disposed, app.lang)}
              {:else}<button class="quiet" onclick={() => (leaving = { asset, date: today(), proceeds: "" })}>{t("assetLeaves")}</button>{/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>
