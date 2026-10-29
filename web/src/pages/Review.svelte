<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<script lang="ts">
  import { api } from "../lib/api";
  import { app, t } from "../lib/state.svelte";

  let issues = $state<string[]>([]);

  $effect(() => {
    const books = app.books;
    const lang = app.lang;
    void app.version;
    if (!books) return;
    api
      .GET("/api/v1/books/{books}/review", { params: { path: { books }, query: { lang } } })
      .then(({ data }) => (issues = data ?? []));
  });
</script>

<section class="wrap">
  <h1>{t("review")}</h1>
  {#if issues.length === 0}
    <p>{t("nothingToReview")}</p>
  {:else}
    <ul>{#each issues as issue, i (i)}<li>{issue}</li>{/each}</ul>
  {/if}
</section>

<style>
  li { white-space: normal; margin-bottom: 0.4rem; }
</style>
