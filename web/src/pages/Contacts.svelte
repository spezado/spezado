<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Contacts: the customers and suppliers of the owner, with their numbers, contact persons and bank account. -->
<script lang="ts">
  import { api, problem, type Schemas } from "../lib/api";
  import { app, t } from "../lib/state.svelte";

  type Contact = Schemas["Contact"];
  let contacts = $state<Contact[]>([]);
  let form = $state<(Contact & { kind: string; persons: Schemas["ContactPerson"][] }) | null>(null);
  let failure = $state<string | null>(null);
  let message = $state<string | null>(null);

  $effect(() => {
    const books = app.books;
    void app.version;
    if (!books) return;
    api.GET("/api/v1/books/{books}/contacts", { params: { path: { books } } }).then(({ data }) => (contacts = data ?? []));
  });

  const kindOf = (c: Contact) => (c.customer && c.supplier ? "both" : c.supplier ? "supplier" : "customer");
  const kindText = (c: Contact) =>
    t(({ both: "customerAndSupplier", supplier: "supplier", customer: "customer" } as const)[kindOf(c)]);

  function edit(contact: Contact | null) {
    failure = message = null;
    const blank: Contact = { id: "", name: "", customer: true, supplier: false, country: "DE", persons: [] };
    const c = contact ?? blank;
    form = { ...c, persons: (c.persons ?? []).map((p) => ({ ...p })), kind: kindOf(c) };
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books || !form) return;
    const { kind, ...rest } = form;
    const optional = (value: string | null | undefined) => (value && value.trim() ? value.trim() : null);
    const body: Contact = {
      ...rest,
      customer: kind !== "supplier",
      supplier: kind !== "customer",
      number: optional(rest.number),
      vat_id: optional(rest.vat_id),
      tax_number: optional(rest.tax_number),
      iban: optional(rest.iban),
      bic: optional(rest.bic),
      reference: optional(rest.reference),
      persons: rest.persons.filter((p) => p.name.trim()),
    };
    const { error, response } = await api.POST("/api/v1/books/{books}/contacts", { params: { path: { books: app.books } }, body });
    if (!response.ok) return void (failure = problem(error, response));
    message = `${t("contactSaved")}: ${body.name}`;
    form = null;
  }
</script>

<section class="wrap">
  <h1>{t("contacts")}</h1>
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}

  {#if form}
    <form class="panel" onsubmit={save}>
      <h2>{form.id ? t("editContact") : t("newContact")}</h2>
      <div class="grid">
        <label>{t("name")} <input bind:value={form.name} required /></label>
        <label>{t("kind")}
          <select bind:value={form.kind}>
            <option value="customer">{t("customer")}</option>
            <option value="supplier">{t("supplier")}</option>
            <option value="both">{t("customerAndSupplier")}</option>
          </select>
        </label>
        <label>{t("number")} <input bind:value={form.number} placeholder={t("numberHint")} /></label>
        <label>{t("street")} <input bind:value={form.street} /></label>
        <label>{t("postcode")} <input bind:value={form.postcode} /></label>
        <label>{t("city")} <input bind:value={form.city} /></label>
        <label>{t("country")} <input bind:value={form.country} maxlength="2" /></label>
        <label>{t("email")} <input type="email" bind:value={form.email} /></label>
        <label>{t("phone")} <input bind:value={form.phone} /></label>
        <label>{t("vatId")} <input bind:value={form.vat_id} /></label>
        <label>{t("taxNumber")} <input bind:value={form.tax_number} /></label>
        <label>{t("iban")} <input bind:value={form.iban} /></label>
        <label>{t("bic")} <input bind:value={form.bic} /></label>
        <label>{t("buyerReference")} <input bind:value={form.reference} /></label>
      </div>
      <h3>{t("contactPersons")}</h3>
      {#each form.persons as person, i (i)}
        <fieldset class="grid">
          <label>{t("name")} <input bind:value={person.name} /></label>
          <label>{t("email")} <input type="email" bind:value={person.email} /></label>
          <label>{t("phone")} <input bind:value={person.phone} /></label>
        </fieldset>
      {/each}
      <div class="row-actions">
        <button type="button" class="quiet" onclick={() => form!.persons.push({ id: "", name: "", email: "", phone: "" })}>+ {t("addPerson")}</button>
      </div>
      <div class="row-actions">
        <button type="button" class="quiet" onclick={() => (form = null)}>{t("cancel")}</button>
        <button type="submit">{t("keep")}</button>
      </div>
    </form>
  {:else}
    <div class="row-actions"><button onclick={() => edit(null)}>+ {t("newContact")}</button></div>
  {/if}

  {#if contacts.length === 0}
    <p class="muted">{t("noContacts")}</p>
  {:else}
    <table class="wide">
      <thead><tr><th>{t("number")}</th><th>{t("name")}</th><th>{t("kind")}</th><th>{t("vatId")}</th><th>{t("city")}</th></tr></thead>
      <tbody>
        {#each contacts as contact (contact.id)}
          <tr>
            <td class="mono">{contact.number ?? ""}</td>
            <td><button class="quiet" onclick={() => edit(contact)}>{contact.name}</button></td>
            <td>{kindText(contact)}</td>
            <td class="mono">{contact.vat_id ?? ""}</td>
            <td>{contact.city}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>
