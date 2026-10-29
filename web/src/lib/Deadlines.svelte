<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- What moves the deadlines: the state whose public holidays apply and the permanent extension of VAT returns,
     changed from a date on. -->
<script lang="ts">
  import { api, problem, type Schemas } from "./api";
  import { today } from "./format";
  import { app, t } from "./state.svelte";

  let current = $state<Schemas["Deadlines"] | null>(null);
  let regions = $state<Schemas["RegionView"][]>([]);
  let form = $state<{ from: string; region: string; vat_extended: boolean } | null>(null);
  let failure = $state<string | null>(null);

  $effect(() => {
    const [books, lang] = [app.books, app.lang];
    void app.version;
    if (!books) return;
    api.GET("/api/v1/books/{books}/deadlines", { params: { path: { books } } }).then(({ data }) => (current = data ?? null));
    api.GET("/api/v1/regions", { params: { query: { lang } } }).then(({ data }) => (regions = data ?? []));
  });

  const regionName = (code: string | null | undefined) =>
    code ? (regions.find((region) => region.code === code)?.name ?? code) : t("noRegion");

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books || !form) return;
    failure = null;
    const result = await api.PUT("/api/v1/books/{books}/deadlines", {
      params: { path: { books: app.books } },
      body: { from: form.from, region: form.region || null, vat_extended: form.vat_extended },
    });
    if (!result.response.ok) return void (failure = problem(result.error, result.response));
    form = null;
    app.version++;
  }
</script>

{#if current}
  <p>
    {t("region")}: {regionName(current.region)} · {t("vatExtended")}: {current.vat_extended ? t("yes") : t("no")}
    {#if !form}
      <button class="quiet" onclick={() => (form = { from: today(), region: current?.region ?? "", vat_extended: current?.vat_extended ?? false })}>{t("change")}</button>
    {/if}
  </p>
{/if}
{#if form}
  <form class="card" onsubmit={submit}>
    <label>{t("validFrom")} <input type="date" bind:value={form.from} required /></label>
    <label>{t("region")}
      <select bind:value={form.region}>
        <option value="">{t("noRegion")}</option>
        {#each regions as region (region.code)}<option value={region.code}>{region.name}</option>{/each}
      </select>
    </label>
    <label class="check"><input type="checkbox" bind:checked={form.vat_extended} /> {t("vatExtended")}</label>
    {#if failure}<p class="error" role="alert">{failure}</p>{/if}
    <div class="row-actions">
      <button type="button" class="quiet" onclick={() => (form = null)}>{t("cancel")}</button>
      <button type="submit">{t("apply")}</button>
    </div>
  </form>
{/if}
