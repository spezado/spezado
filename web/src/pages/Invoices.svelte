<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Invoices: the seller's details, the customers, and a new invoice written as XRechnung – recorded as a document,
     booked when paid on a cash basis, when issued with double entry. -->
<script lang="ts">
  import { api, download, problem, type Schemas } from "../lib/api";
  import { amount, parseAmount, today } from "../lib/format";
  import { app, t } from "../lib/state.svelte";

  type Profile = Schemas["InvoiceProfile"];
  type Customer = Schemas["Customer"];
  const blankProfile = (): Profile => ({
    name: "", street: "", postcode: "", city: "", country: "DE", vat_id: "", tax_number: "", contact: "", phone: "",
    email: "", iban: "", payment_days: 14, prefix: "RE-",
  });
  const blankCustomer = (): Customer => ({
    id: "", name: "", street: "", postcode: "", city: "", country: "DE", email: "", vat_id: "", reference: "",
  });
  const blankLine = () => ({ name: "", quantity: "1", unit: "HUR", price: "", tax: "de-out-19" });

  let profile = $state<Profile | null>(null);
  let editingProfile = $state(false);
  let profileForm = $state<Profile>(blankProfile());
  let customers = $state<Customer[]>([]);
  let customerForm = $state<Customer | null>(null);
  let next = $state("");
  let invoice = $state({ customer: "", date: today(), delivery: "", note: "", exemption: "", format: "xrechnung", lines: [blankLine()] });
  let failure = $state<string | null>(null);
  let issued = $state<Schemas["Issued"] | null>(null);

  $effect(() => {
    const books = app.books;
    void app.version;
    if (!books) return;
    api.GET("/api/v1/books/{books}/invoicing/profile", { params: { path: { books } } }).then(({ data }) => {
      profile = data ?? null;
      if (!profile) editingProfile = true;
    });
    api.GET("/api/v1/books/{books}/customers", { params: { path: { books } } }).then(({ data }) => (customers = data ?? []));
    api.GET("/api/v1/books/{books}/invoices/next-number", { params: { path: { books }, query: { year: Number(invoice.date.slice(0, 4)) } } })
      .then(({ data }) => (next = data?.number ?? ""));
  });

  async function saveProfile(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books) return;
    failure = null;
    const body = { ...profileForm, vat_id: profileForm.vat_id || null, tax_number: profileForm.tax_number || null };
    const result = await api.PUT("/api/v1/books/{books}/invoicing/profile", { params: { path: { books: app.books } }, body });
    if (!result.response.ok) return void (failure = problem(result.error, result.response));
    profile = result.data ?? null;
    editingProfile = false;
  }

  async function saveCustomer(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books || !customerForm) return;
    failure = null;
    const body = { ...customerForm, vat_id: customerForm.vat_id || null, reference: customerForm.reference || null };
    const result = await api.POST("/api/v1/books/{books}/customers", { params: { path: { books: app.books } }, body });
    if (!result.response.ok) return void (failure = problem(result.error, result.response));
    invoice.customer = result.data!.id;
    customerForm = null;
    app.version++;
  }

  async function issue(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books) return;
    failure = null;
    issued = null;
    const lines = invoice.lines.map((line) => ({ ...line, price: parseAmount(line.price, app.lang), quantity: parseAmount(line.quantity, app.lang) }));
    if (lines.some((line) => line.price === null || line.quantity === null)) return void (failure = `${t("price")}?`);
    const result = await api.POST("/api/v1/books/{books}/invoices", {
      params: { path: { books: app.books } },
      body: {
        customer: invoice.customer,
        date: invoice.date,
        delivery: invoice.delivery || null,
        note: invoice.note || null,
        exemption: invoice.exemption || null,
        format: invoice.format,
        lines: lines.map((line) => ({ name: line.name, quantity: line.quantity!, unit: line.unit, price: line.price!, tax: line.tax || null })),
      },
    });
    if (!result.response.ok) return void (failure = problem(result.error, result.response));
    issued = result.data ?? null;
    invoice = { customer: invoice.customer, date: invoice.date, delivery: "", note: "", exemption: "", format: invoice.format, lines: [blankLine()] };
  }
</script>

<section class="wrap">
  <h1>{t("invoices")}</h1>
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}

  {#if editingProfile}
    <form class="card" onsubmit={saveProfile}>
      <h2>{t("seller")}</h2>
      <p class="hint">{t("sellerHint")}</p>
      <div class="grid">
        <label>{t("name")} <input bind:value={profileForm.name} required /></label>
        <label>{t("street")} <input bind:value={profileForm.street} required /></label>
        <label>{t("postcode")} <input bind:value={profileForm.postcode} required /></label>
        <label>{t("city")} <input bind:value={profileForm.city} required /></label>
        <label>{t("vatId")} <input bind:value={profileForm.vat_id} /></label>
        <label>{t("taxNumber")} <input bind:value={profileForm.tax_number} /></label>
        <label>{t("contact")} <input bind:value={profileForm.contact} required /></label>
        <label>{t("phone")} <input bind:value={profileForm.phone} required /></label>
        <label>{t("email")} <input type="email" bind:value={profileForm.email} required /></label>
        <label>IBAN <input bind:value={profileForm.iban} required /></label>
        <label>{t("paymentDays")} <input type="number" min="0" bind:value={profileForm.payment_days} required /></label>
        <label>{t("numberPrefix")} <input bind:value={profileForm.prefix} required /></label>
      </div>
      <div class="row-actions">
        {#if profile}<button type="button" class="quiet" onclick={() => (editingProfile = false)}>{t("cancel")}</button>{/if}
        <button type="submit">{t("keep")}</button>
      </div>
    </form>
  {:else if profile}
    <p class="muted">{t("seller")}: {profile.name}, {profile.city} ·
      <button class="quiet" onclick={() => { profileForm = { ...profile!, vat_id: profile!.vat_id ?? "", tax_number: profile!.tax_number ?? "" }; editingProfile = true; }}>{t("change")}</button>
    </p>

    {#if customerForm}
      <form class="card" onsubmit={saveCustomer}>
        <h2>{t("newCustomer")}</h2>
        <div class="grid">
          <label>{t("name")} <input bind:value={customerForm.name} required /></label>
          <label>{t("street")} <input bind:value={customerForm.street} required /></label>
          <label>{t("postcode")} <input bind:value={customerForm.postcode} required /></label>
          <label>{t("city")} <input bind:value={customerForm.city} required /></label>
          <label>{t("country")} <input bind:value={customerForm.country} required maxlength="2" /></label>
          <label>{t("email")} <input type="email" bind:value={customerForm.email} required /></label>
          <label>{t("vatId")} <input bind:value={customerForm.vat_id} /></label>
          <label>{t("buyerReference")} <input bind:value={customerForm.reference} /></label>
        </div>
        <div class="row-actions">
          <button type="button" class="quiet" onclick={() => (customerForm = null)}>{t("cancel")}</button>
          <button type="submit">{t("keep")}</button>
        </div>
      </form>
    {:else}
      <form class="card" onsubmit={issue}>
        <h2>{t("newInvoice")} {next}</h2>
        <div class="grid">
          <label>{t("customer")}
            <select bind:value={invoice.customer} required>
              <option value="" disabled>–</option>
              {#each customers as customer (customer.id)}<option value={customer.id}>{customer.name}</option>{/each}
            </select>
          </label>
          <div class="row-actions"><button type="button" class="quiet" onclick={() => (customerForm = blankCustomer())}>+ {t("newCustomer")}</button></div>
          <label>{t("date")} <input type="date" bind:value={invoice.date} required /></label>
          <label>{t("deliveryDate")} <input type="date" bind:value={invoice.delivery} /></label>
        </div>
        {#each invoice.lines as line, i (i)}
          <fieldset>
            <label>{t("service")} <input bind:value={line.name} required /></label>
            <label>{t("quantity")} <input inputmode="decimal" bind:value={line.quantity} required /></label>
            <label>{t("unit")}
              <select bind:value={line.unit}>
                <option value="HUR">{t("hours")}</option>
                <option value="DAY">{t("days")}</option>
                <option value="C62">{t("pieces")}</option>
              </select>
            </label>
            <label>{t("price")} <input inputmode="decimal" bind:value={line.price} required /></label>
            <label>{t("taxCode")}
              <select bind:value={line.tax}>
                <option value="de-out-19">19 %</option>
                <option value="de-out-7">7 %</option>
                <option value="de-out-0">0 %</option>
                <option value="de-out-exempt">{t("taxExempt")}</option>
                <option value="de-out-13b">{t("tax13b")}</option>
                <option value="de-out-eu-service">{t("taxEuService")}</option>
                <option value="de-out-ig-supply">{t("taxIgSupply")}</option>
                <option value="de-out-export">{t("taxExport")}</option>
                <option value="">{t("smallBusinessLine")}</option>
              </select>
            </label>
            {#if invoice.lines.length > 1}
              <button type="button" class="quiet" onclick={() => invoice.lines.splice(i, 1)}>{t("remove")}</button>
            {/if}
          </fieldset>
        {/each}
        {#if invoice.lines.some((line) => line.tax === "de-out-exempt")}
          <label>{t("exemptionReason")} <input bind:value={invoice.exemption} placeholder="Steuerfrei nach § 4 Nr. … UStG" /></label>
        {/if}
        <label>{t("note")} <input bind:value={invoice.note} /></label>
        <label>{t("invoiceFile")}
          <select bind:value={invoice.format}>
            <option value="xrechnung">XRechnung (XML)</option>
            <option value="zugferd">ZUGFeRD (PDF)</option>
          </select>
        </label>
        <div class="row-actions">
          <button type="button" class="quiet" onclick={() => invoice.lines.push(blankLine())}>{t("addLine")}</button>
          <button type="submit">{t("writeInvoice")}</button>
        </div>
      </form>
    {/if}

    {#if issued}
      <p role="status">
        {t("invoiceWritten")}: {issued.number} · {amount(issued.due.value, app.lang)} {issued.due.currency} ·
        <button class="quiet" onclick={async () => (failure = await download(`/api/v1/books/${app.books}/documents/${issued!.document}/original`, issued!.name))}>{issued.name}</button>
      </p>
    {/if}
  {/if}
</section>

<style>
  .grid { display: grid; gap: 0.6rem; grid-template-columns: repeat(auto-fill, minmax(14rem, 1fr)); align-items: end; }
  fieldset { border: 1px solid var(--line); border-radius: 6px; display: grid; gap: 0.6rem; padding: 0.75rem; margin: 0.75rem 0;
    grid-template-columns: repeat(auto-fill, minmax(10rem, 1fr)); align-items: end; }
  .hint { color: var(--muted); }
  form { display: grid; gap: 0.75rem; }
</style>
