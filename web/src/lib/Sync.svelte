<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Synchronisation of the books with the person's other devices through a relay: starting it (and the recovery
     code, shown once), the devices, inviting another one, and synchronising now. -->
<script lang="ts">
  import { api, problem, type Schemas } from "./api";
  import { app, t } from "./state.svelte";

  let relay = $state<string | null | undefined>(undefined);
  let devices = $state<Schemas["SyncDevice"][]>([]);
  let start = $state({ relay: "", name: "" });
  let code = $state<string | null>(null);
  let invitation = $state<string | null>(null);
  let report = $state<Schemas["SyncReport"] | null>(null);
  let busy = $state(false);
  let rights = $state("manage");
  let failure = $state<string | null>(null);

  $effect(() => {
    const books = app.books;
    void app.version;
    if (!books) return;
    api.GET("/api/v1/books/{books}/sync", { params: { path: { books } } }).then(({ data }) => {
      relay = data?.relay ?? null;
      if (relay) api.GET("/api/v1/books/{books}/sync/devices", { params: { path: { books } } }).then(({ data }) => (devices = data ?? []));
    });
  });

  async function begin(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books) return;
    busy = true;
    failure = null;
    const result = await api.POST("/api/v1/books/{books}/sync/start", {
      params: { path: { books: app.books } },
      body: { relay: start.relay, name: start.name || null },
    });
    busy = false;
    if (!result.response.ok) return void (failure = problem(result.error, result.response));
    code = result.data!.code;
  }

  async function now() {
    if (!app.books) return;
    busy = true;
    failure = null;
    const result = await api.POST("/api/v1/books/{books}/sync", { params: { path: { books: app.books } } });
    busy = false;
    if (!result.response.ok) return void (failure = problem(result.error, result.response));
    report = result.data ?? null;
    if (report?.added.length) invitation = null;
    app.version++;
  }

  async function remove(device: Schemas["SyncDevice"]) {
    if (!app.books || !confirm(`${t("removeConfirm")} ${device.name}`)) return;
    failure = null;
    const result = await api.DELETE("/api/v1/books/{books}/sync/devices/{member}", { params: { path: { books: app.books, member: device.id } } });
    if (!result.response.ok) return void (failure = problem(result.error, result.response));
    app.version++;
  }

  async function invite() {
    if (!app.books) return;
    failure = null;
    const result = await api.POST("/api/v1/books/{books}/sync/invitations", { params: { path: { books: app.books } }, body: { rights } });
    if (!result.response.ok) return void (failure = problem(result.error, result.response));
    invitation = result.data!.invitation;
  }
</script>

<section class="sync">
  <h2>{t("sync")}</h2>
  {#if code}
    <div class="card code" role="alert">
      <p><strong>{t("recoveryCode")}</strong></p>
      <p class="mono">{code}</p>
      <p>{t("recoveryHint")}</p>
      <button onclick={() => { code = null; app.version++; }}>{t("recoveryNoted")}</button>
    </div>
  {:else if relay === null}
    <p class="muted">{t("syncIntro")}</p>
    <form class="card" onsubmit={begin}>
      <label>{t("relay")} <input type="url" bind:value={start.relay} placeholder="https://relay.example.org" required /></label>
      <label>{t("deviceName")} <input bind:value={start.name} placeholder="Laptop" /></label>
      <div class="row-actions"><button type="submit" disabled={busy}>{t("syncStart")}</button></div>
    </form>
  {:else if relay}
    <p>{t("syncedThrough")} <span class="mono">{relay}</span></p>
    <ul>
      {#each devices as device (device.name + (device.device ?? ""))}
        <li>
          {device.recovery ? t("recoveryCode") : device.name}{device.me ? ` (${t("thisDevice")})` : ""}
          {#if !device.recovery}<span class="muted">· {t(`rights_${device.rights as "read" | "write" | "manage"}`)}</span>{/if}
          {#if !device.me && !device.recovery}<button class="quiet" onclick={() => remove(device)}>{t("remove")}</button>{/if}
        </li>
      {/each}
    </ul>
    {#if report}
      <p class="muted">{t("syncReport")}: {report.received} ↓ · {report.sent} ↑{report.added.length ? ` · ${t("added")}: ${report.added.join(", ")}` : ""}</p>
    {/if}
    {#if invitation}
      <div class="card">
        <p>{t("invitationHint")}</p>
        <p class="mono invitation">{invitation}</p>
      </div>
    {/if}
    <div class="row-actions">
      <label>{t("inviteRights")}
        <select bind:value={rights}>
          <option value="manage">{t("rights_manage")}</option>
          <option value="write">{t("rights_write")}</option>
          <option value="read">{t("rights_read")}</option>
        </select>
      </label>
      <button class="quiet" onclick={invite}>{t("inviteDevice")}</button>
      <button onclick={now} disabled={busy}>{t("syncNow")}</button>
    </div>
  {/if}
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
</section>

<style>
  .mono { font-family: var(--mono, ui-monospace, monospace); overflow-wrap: anywhere; }
  .code { border-left: 3px solid var(--accent); }
  .invitation { user-select: all; }
</style>
