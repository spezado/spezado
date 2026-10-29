<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
  import { untrack } from "svelte";
  import { api, problem, setToken } from "../lib/api";
  import { t } from "../lib/state.svelte";

  let { code: given = "", onpaired }: { code?: string; onpaired: () => void } = $props();
  let code = $state(untrack(() => given));
  let name = $state(`Browser ${navigator.platform}`.trim());
  let failure = $state<string | null>(null);

  async function pair(event: SubmitEvent) {
    event.preventDefault();
    const { data, error, response } = await api.POST("/api/v1/pair", { body: { code: code.trim(), name } });
    if (!data) {
      failure = problem(error, response);
      return;
    }
    setToken(data.token);
    history.replaceState(null, "", location.pathname);
    onpaired();
  }

  // A code in the address pairs at once.
  $effect(() => {
    if (given) pair(new SubmitEvent("submit"));
  });
</script>

<section class="wrap pair">
  <h1>{t("pairTitle")}</h1>
  <p class="muted">{t("pairText")}</p>
  <form onsubmit={pair}>
    <label>{t("code")} <input bind:value={code} required autocomplete="one-time-code" /></label>
    <label>{t("deviceName")} <input bind:value={name} required /></label>
    {#if failure}<p class="error" role="alert">{failure}</p>{/if}
    <div class="row-actions"><button type="submit">{t("pair")}</button></div>
  </form>
</section>

<style>
  .pair { padding-top: 3rem; max-width: 40rem; }
</style>
