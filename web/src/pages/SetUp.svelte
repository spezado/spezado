<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
  import { api, problem, type Schemas } from "../lib/api";
  import { app, t } from "../lib/state.svelte";

  let { onsetup }: { onsetup: () => void } = $props();
  const year = new Date().getFullYear();
  let form = $state({
    name: "",
    from: `${year}-01-01`,
    accounting: "de-cash-basis",
    vat_scheme: "de-regular",
    vat_basis: "cash",
    vat_period: "quarter",
    chart: "de-skr03",
    vat_extended: false,
    region: "",
  });
  let regions = $state<Schemas["RegionView"][]>([]);

  $effect(() => {
    const lang = app.lang;
    api.GET("/api/v1/regions", { params: { query: { lang } } }).then(({ data }) => (regions = data ?? []));
  });
  let failure = $state<string | null>(null);

  let joining = $state({ invitation: "", name: "" });
  let recovering = $state({ relay: "", books: "", code: "", name: "" });
  let joined = $state(false);
  let otherFailure = $state<string | null>(null);

  async function join(event: SubmitEvent) {
    event.preventDefault();
    otherFailure = null;
    const { data, error, response } = await api.POST("/api/v1/sync/join", { body: { invitation: joining.invitation, name: joining.name || null } });
    if (!data) otherFailure = problem(error, response);
    else joined = true;
  }

  async function recover(event: SubmitEvent) {
    event.preventDefault();
    otherFailure = null;
    const { data, error, response } = await api.POST("/api/v1/sync/recover", { body: { ...recovering, name: recovering.name || null } });
    if (!data) otherFailure = problem(error, response);
    else onsetup();
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const { data, error, response } = await api.POST("/api/v1/books", { body: { ...form, region: form.region || null } });
    if (!data) failure = problem(error, response);
    else onsetup();
  }
</script>

<section class="wrap page">
  <h1>{t("setUpTitle")}</h1>
  <form onsubmit={submit}>
    <label>{t("name")} <input bind:value={form.name} required /></label>
    <label>{t("start")} <input type="date" bind:value={form.from} required /></label>
    <label>{t("accounting")}
      <select bind:value={form.accounting}>
        <option value="de-cash-basis">{t("cashBasis")}</option>
        <option value="de-double-entry">{t("doubleEntry")}</option>
      </select>
    </label>
    <label>{t("vat")}
      <select bind:value={form.vat_scheme}>
        <option value="de-regular">{t("regular")}</option>
        <option value="de-small-business">{t("smallBusiness")}</option>
      </select>
    </label>
    {#if form.vat_scheme === "de-regular"}
      <label>{t("vatBasis")}
        <select bind:value={form.vat_basis}>
          <option value="cash">{t("cashVat")}</option>
          <option value="accrual">{t("accrual")}</option>
        </select>
      </label>
      <label>{t("vatPeriod")}
        <select bind:value={form.vat_period}>
          <option value="quarter">{t("quarterly")}</option>
          <option value="month">{t("monthly")}</option>
          <option value="year">{t("yearly")}</option>
        </select>
      </label>
      <label class="check"><input type="checkbox" bind:checked={form.vat_extended} /> {t("vatExtended")}</label>
    {/if}
    <label>{t("region")}
      <select bind:value={form.region}>
        <option value="">{t("noRegion")}</option>
        {#each regions as region (region.code)}<option value={region.code}>{region.name}</option>{/each}
      </select>
    </label>
    <label>{t("chart")}
      <select bind:value={form.chart}>
        <option value="de-skr03">SKR 03</option>
        <option value="de-skr04">SKR 04</option>
      </select>
    </label>
    {#if failure}<p class="error" role="alert">{failure}</p>{/if}
    <div class="row-actions"><button type="submit">{t("setUp")}</button></div>
  </form>

  <h2>{t("fromOtherDevice")}</h2>
  {#if joined}
    <p role="status">{t("joinWaiting")}</p>
  {:else}
    <form onsubmit={join}>
      <label>{t("invitation")} <input bind:value={joining.invitation} required /></label>
      <label>{t("deviceName")} <input bind:value={joining.name} placeholder="Laptop" /></label>
      <div class="row-actions"><button type="submit">{t("join")}</button></div>
    </form>
    <details>
      <summary>{t("recover")}</summary>
      <form onsubmit={recover}>
        <label>{t("relay")} <input type="url" bind:value={recovering.relay} required /></label>
        <label>{t("booksId")} <input bind:value={recovering.books} required /></label>
        <label>{t("recoveryCode")} <input bind:value={recovering.code} required autocomplete="off" /></label>
        <label>{t("deviceName")} <input bind:value={recovering.name} /></label>
        <div class="row-actions"><button type="submit">{t("recoverAction")}</button></div>
      </form>
    </details>
  {/if}
  {#if otherFailure}<p class="error" role="alert">{otherFailure}</p>{/if}
</section>

<style>
  .page { padding-top: 2.5rem; }
  .check { display: flex; gap: 0.5rem; align-items: center; }
</style>
