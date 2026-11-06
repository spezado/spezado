<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Payroll (librotenado 0013): the employees, a payslip computed and then recorded and booked, the payslips of the
     year with their corrections, the Beitragsnachweis of a month and the Lohnsteuerbescheinigung of a year. -->
<script lang="ts">
  import { api, problem, type Schemas } from "../lib/api";
  import { amount, date, parseAmount, today } from "../lib/format";
  import { app, t } from "../lib/state.svelte";

  type Employment = Schemas["EmploymentView"];
  type Payslip = Schemas["PayslipView"];
  let employments = $state<Employment[]>([]);
  let payslips = $state<Payslip[]>([]);
  let failure = $state<string | null>(null);
  let message = $state<string | null>(null);

  const blank = () => ({
    name: "", start: today(), born: "", class: "1", children: "", church: "", insurer: "", contribution_group: "1111",
    person_group: "101", additional_rate: "2.9", childless: false, care_children: 0, saxony: false, u1: "", u2: "",
    flat: false, short_term: false, household: false, active_pension: false,
  });
  let hiring = $state<ReturnType<typeof blank> | null>(null);

  const items = ["wage", "one_off", "in_kind", "tax_free", "deduction", "pension"] as const;
  const itemText = { wage: "wage", one_off: "oneOff", in_kind: "inKind", tax_free: "taxFree", deduction: "deduction", pension: "pensionPaid" } as const;
  let slip = $state<{ employment: string; month: string; corrects: string | null; paid: string; amounts: Record<string, string> }>({
    employment: "", month: today().slice(0, 7), corrects: null, paid: today(), amounts: {},
  });
  let preview = $state<Payslip | null>(null);

  let month = $state(today().slice(0, 7));
  let statements = $state<{ insurer: string; fields: Record<string, string> }[]>([]);
  let certificateOf = $state("");
  let certificate = $state<Record<string, string> | null>(null);

  $effect(() => {
    const [books, year] = [app.books, app.year];
    void app.version;
    if (!books) return;
    api.GET("/api/v1/books/{books}/employments", { params: { path: { books } } }).then(({ data }) => {
      employments = data ?? [];
      if (!slip.employment && employments[0]) slip.employment = employments[0].id;
      if (!certificateOf && employments[0]) certificateOf = employments[0].id;
    });
    api.GET("/api/v1/books/{books}/payslips", { params: { path: { books }, query: { year } } }).then(({ data }) => (payslips = data ?? []));
  });

  const nameOf = (id: string) => employments.find((e) => e.id === id)?.name ?? id;

  async function hire(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books || !hiring) return;
    failure = message = null;
    const h = hiring;
    const body = {
      name: h.name, start: h.start, born: h.born || null, class: h.flat || h.short_term ? null : Number(h.class),
      children: h.children || null, church: h.church || null, insurer: h.insurer, contribution_group: h.contribution_group,
      person_group: h.person_group, additional_rate: h.additional_rate || null, childless: h.childless,
      care_children: Number(h.care_children), saxony: h.saxony, u1: h.u1 || null, u2: h.u2 || null, flat: h.flat,
      short_term: h.short_term, household: h.household, active_pension: h.active_pension,
    };
    const { error, response } = await api.POST("/api/v1/books/{books}/employments", { params: { path: { books: app.books } }, body });
    if (!response.ok) return void (failure = problem(error, response));
    message = `${t("newEmployee")}: ${h.name}`;
    hiring = null;
  }

  function payslipBody(record: boolean) {
    const list = items
      .filter((kind) => slip.amounts[kind])
      .map((kind) => {
        const text = slip.amounts[kind] ?? "";
        return { kind: kind as string, name: t(itemText[kind]), amount: parseAmount(text, app.lang) ?? text };
      });
    return { employment: slip.employment, month: slip.month, items: list, record, corrects: slip.corrects, paid: slip.corrects ? slip.paid : null };
  }

  async function compute(record: boolean) {
    if (!app.books) return;
    failure = message = null;
    const { data, error, response } = await api.POST("/api/v1/books/{books}/payslips", {
      params: { path: { books: app.books } },
      body: payslipBody(record),
    });
    if (!response.ok) return void (failure = problem(error, response));
    preview = data ?? null;
    if (record) {
      message = t("payslipRecorded");
      slip.corrects = null;
      slip.amounts = {};
    }
  }

  function correct(p: Payslip) {
    slip.employment = p.employment;
    slip.month = p.to.slice(0, 7);
    slip.corrects = p.id;
    slip.paid = today();
    slip.amounts = Object.fromEntries(p.items.map((i) => [i.kind, amount(i.amount, app.lang)]));
    preview = null;
  }

  async function contributions() {
    if (!app.books) return;
    const [y = 0, m = 1] = month.split("-").map(Number);
    const from = `${month}-01`;
    const to = `${month}-${String(new Date(y, m, 0).getDate()).padStart(2, "0")}`;
    const insurers = [...new Set(employments.map((e) => e.insurer).filter((i): i is string => !!i))];
    statements = [];
    for (const insurer of insurers) {
      const { data } = await api.GET("/api/v1/books/{books}/contributions", { params: { path: { books: app.books }, query: { insurer, from, to } } });
      statements.push({ insurer, fields: (data ?? {}) as Record<string, string> });
    }
  }

  async function showCertificate() {
    if (!app.books || !certificateOf) return;
    const { data } = await api.GET("/api/v1/books/{books}/employments/{employment}/certificate", {
      params: { path: { books: app.books, employment: certificateOf }, query: { year: app.year } },
    });
    certificate = (data ?? {}) as Record<string, string>;
  }

  const figure = (p: Payslip, name: string) => (p.figures[name] ? amount(p.figures[name], app.lang) : "–");
</script>

<section class="wrap">
  <h1>{t("payroll")}</h1>
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}

  <h2>{t("employees")}</h2>
  <div class="row-actions"><button onclick={() => (hiring = blank())}>+ {t("newEmployee")}</button></div>
  {#if hiring}
    <form class="panel" onsubmit={hire}>
      <h2>{t("newEmployee")}</h2>
      <div class="grid">
        <label>{t("name")} <input bind:value={hiring.name} required /></label>
        <label>{t("start")} <input type="date" bind:value={hiring.start} required /></label>
        <label>{t("born")} <input type="date" bind:value={hiring.born} /></label>
        <label>{t("taxClass")} <select bind:value={hiring.class}>{#each ["1", "2", "3", "4", "5", "6"] as c (c)}<option>{c}</option>{/each}</select></label>
        <label>{t("childAllowances")} <input bind:value={hiring.children} placeholder="1.5" /></label>
        <label>{t("church")} <select bind:value={hiring.church}><option value="">{t("noChurch")}</option><option value="ev">ev</option><option value="rk">rk</option><option value="ak">ak</option></select></label>
        <label>{t("insurer")} <input bind:value={hiring.insurer} required pattern="[0-9]{8}" /></label>
        <label>{t("contributionGroups")} <input bind:value={hiring.contribution_group} required pattern="[0-9]{4}" /></label>
        <label>{t("personGroup")} <input bind:value={hiring.person_group} required pattern="[0-9]{3}" /></label>
        <label>{t("additionalRate")} <input bind:value={hiring.additional_rate} /></label>
        <label>{t("careChildren")} <input type="number" min="0" bind:value={hiring.care_children} /></label>
        <label>U1 % <input bind:value={hiring.u1} /></label>
        <label>U2 % <input bind:value={hiring.u2} /></label>
      </div>
      <div class="checks">
        <label><input type="checkbox" bind:checked={hiring.childless} /> {t("childless")}</label>
        <label><input type="checkbox" bind:checked={hiring.saxony} /> {t("saxony")}</label>
        <label><input type="checkbox" bind:checked={hiring.flat} /> {t("flatTax")}</label>
        <label><input type="checkbox" bind:checked={hiring.short_term} /> {t("shortTerm")}</label>
        <label><input type="checkbox" bind:checked={hiring.household} /> {t("household")}</label>
        <label><input type="checkbox" bind:checked={hiring.active_pension} /> {t("activePension")}</label>
      </div>
      <div class="row-actions">
        <button type="button" class="quiet" onclick={() => (hiring = null)}>{t("cancel")}</button>
        <button type="submit">{t("keep")}</button>
      </div>
    </form>
  {/if}
  {#if employments.length === 0}
    <p class="muted">{t("noEmployees")}</p>
  {:else}
    <table class="wide">
      <thead><tr><th>{t("name")}</th><th>{t("start")}</th><th>{t("taxClass")}</th><th>{t("insurer")}</th><th>{t("contributionGroups")}</th><th>{t("personGroup")}</th></tr></thead>
      <tbody>
        {#each employments as e (e.id)}
          <tr><td>{e.name}</td><td>{date(e.start, app.lang)}</td><td>{e.class ?? "–"}</td><td class="mono">{e.insurer ?? ""}</td><td class="mono">{e.contribution_group ?? ""}</td><td class="mono">{e.person_group ?? ""}</td></tr>
        {/each}
      </tbody>
    </table>

    <h2>{slip.corrects ? `${t("correctionOf")} ${nameOf(slip.employment)} ${slip.month}` : t("payslip")}</h2>
    <form class="panel" onsubmit={(e) => { e.preventDefault(); compute(false); }}>
      <div class="grid">
        <label>{t("employee")} <select bind:value={slip.employment} disabled={!!slip.corrects}>{#each employments as e (e.id)}<option value={e.id}>{e.name}</option>{/each}</select></label>
        <label>{t("month")} <input type="month" bind:value={slip.month} disabled={!!slip.corrects} required /></label>
        {#if slip.corrects}<label>{t("paid")} <input type="date" bind:value={slip.paid} required /></label>{/if}
        {#each items as kind (kind)}
          <label>{t(itemText[kind])} <input inputmode="decimal" bind:value={slip.amounts[kind]} /></label>
        {/each}
      </div>
      <div class="row-actions">
        {#if slip.corrects}<button type="button" class="quiet" onclick={() => { slip.corrects = null; slip.amounts = {}; preview = null; }}>{t("cancel")}</button>{/if}
        <button type="submit" class="quiet">{t("compute")}</button>
        <button type="button" onclick={() => compute(true)}>{t("recordPayslip")}</button>
      </div>
    </form>
    {#if preview}
      <table>
        <tbody>
          {#each Object.entries(preview.figures) as [name, value] (name)}
            <tr><td class="mono">{name}</td><td class="number">{amount(value, app.lang)}</td></tr>
          {/each}
        </tbody>
      </table>
    {/if}

    <h2>{t("payslips")} {app.year}</h2>
    {#if payslips.length === 0}
      <p class="muted">{t("noPayslips")}</p>
    {:else}
      <table class="wide">
        <thead><tr><th>{t("employee")}</th><th>{t("from")}</th><th>{t("to")}</th><th class="number">{t("gross")}</th><th class="number">{t("net")}</th><th class="number">{t("payout")}</th><th></th></tr></thead>
        <tbody>
          {#each payslips as p (p.id)}
            <tr>
              <td>{nameOf(p.employment)}{p.replaces ? ` (${t("correction")})` : ""}</td>
              <td>{date(p.from, app.lang)}</td><td>{date(p.to, app.lang)}</td>
              <td class="number">{figure(p, "gross")}</td><td class="number">{figure(p, "net")}</td><td class="number">{figure(p, "payout")}</td>
              <td><button class="quiet" onclick={() => correct(p)}>{t("correct")}</button></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}

    <h2>{t("contributionStatement")}</h2>
    <div class="row-actions">
      <label>{t("month")} <input type="month" bind:value={month} /></label>
      <button class="quiet" onclick={contributions}>{t("show")}</button>
    </div>
    <p class="muted">{t("contributionHint")}</p>
    {#each statements as s (s.insurer)}
      <h3 class="mono">{s.insurer}</h3>
      <table><tbody>{#each Object.entries(s.fields) as [field, value] (field)}<tr><td class="mono">{field}</td><td class="number">{amount(value, app.lang)}</td></tr>{/each}</tbody></table>
    {/each}

    <h2>{t("certificate")} {app.year}</h2>
    <div class="row-actions">
      <select bind:value={certificateOf}>{#each employments as e (e.id)}<option value={e.id}>{e.name}</option>{/each}</select>
      <button class="quiet" onclick={showCertificate}>{t("show")}</button>
    </div>
    {#if certificate}
      <table><tbody>{#each Object.entries(certificate) as [line, value] (line)}<tr><td class="mono">{line}</td><td class="number">{amount(value, app.lang)}</td></tr>{/each}</tbody></table>
    {/if}
  {/if}
</section>

<style>
  .checks { display: flex; flex-wrap: wrap; gap: 1rem; margin-block: 0.75rem; }
  .checks label { flex-direction: row; align-items: center; gap: 0.4rem; }
</style>
