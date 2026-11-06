<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- What the books do not hold of the taxes of a year – losses carried forward, hidden distributions, a tax group,
     the interest barrier; children, capital income and other income of a person –, and the capital accounts of the
     partners of a partnership for the E-Bilanz. -->
<script lang="ts">
  import { api, problem, type Schemas } from "../lib/api";
  import { parseAmount } from "../lib/format";
  import { app, t } from "../lib/state.svelte";

  /** The amounts of a form as the API reads them – `10.000,00` as `10000.00` –, the other fields as they are. */
  function amounts<T extends object>(facts: T): T {
    const words = ["holding", "group"];
    return Object.fromEntries(
      Object.entries(facts).map(([key, value]) =>
        typeof value === "string" && !words.includes(key) && value !== "" ? [key, parseAmount(value, app.lang) ?? value] : [key, value],
      ),
    ) as T;
  }

  let profit = $state<Schemas["ProfitTaxFacts"] | null>(null);
  let income = $state<Schemas["IncomeTaxFacts"] | null>(null);
  let partners = $state<{ id: string; name: string; accounts: boolean }[]>([]);
  let liability = $state<Record<string, string>>({});
  let failure = $state<string | null>(null);
  let message = $state<string | null>(null);

  $effect(() => {
    const [books, year] = [app.books, app.year];
    void app.version;
    if (!books) return;
    api.GET("/api/v1/books/{books}/profit-tax", { params: { path: { books }, query: { year } } }).then(({ data }) => (profit = data ?? null));
    api.GET("/api/v1/books/{books}/income-tax", { params: { path: { books }, query: { year } } }).then(({ data }) => (income = data ?? null));
    api.GET("/api/v1/books/{books}/partners", { params: { path: { books } } }).then(({ data }) => (partners = (data ?? []) as typeof partners));
  });

  async function saveProfit(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books || !profit) return;
    failure = message = null;
    const { error, response } = await api.PUT("/api/v1/books/{books}/profit-tax", { params: { path: { books: app.books }, query: { year: app.year } }, body: amounts(profit) });
    if (!response.ok) return void (failure = problem(error, response));
    message = `${t("saved")}: ${t("profitTaxFacts")}`;
  }

  async function saveIncome(event: SubmitEvent) {
    event.preventDefault();
    if (!app.books || !income) return;
    failure = message = null;
    const { error, response } = await api.PUT("/api/v1/books/{books}/income-tax", { params: { path: { books: app.books }, query: { year: app.year } }, body: amounts(income) });
    if (!response.ok) return void (failure = problem(error, response));
    message = `${t("saved")}: ${t("incomeTaxFacts")}`;
  }

  async function open(partner: string) {
    if (!app.books) return;
    failure = message = null;
    const { error, response } = await api.POST("/api/v1/books/{books}/partners/{partner}/capital-accounts", {
      params: { path: { books: app.books, partner } },
      body: { liability: liability[partner] ?? "unlimited" },
    });
    if (!response.ok) return void (failure = problem(error, response));
    message = t("accountsOpen");
  }
</script>

<section class="wrap">
  <h1>{t("taxes")} {app.year}</h1>
  {#if failure}<p class="error" role="alert">{failure}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}
  <p class="muted">{t("factsHint")}</p>

  {#if profit}
    <form class="panel" onsubmit={saveProfit}>
      <h2>{t("profitTaxFacts")}</h2>
      <div class="grid">
        <label>{t("incomeLosses")} <input inputmode="decimal" bind:value={profit.income_losses} /></label>
        <label>{t("tradeLosses")} <input inputmode="decimal" bind:value={profit.trade_losses} /></label>
        <label>{t("landReduction")} <input inputmode="decimal" bind:value={profit.land_reduction} /></label>
        <label>{t("turnoverAndWages")} <input inputmode="decimal" bind:value={profit.turnover_and_wages} /></label>
        <label>{t("holding")}
          <select bind:value={profit.holding}>
            <option value="at_least_15">≥ 15 %</option>
            <option value="10_to_15">10 – 15 %</option>
            <option value="below_10">&lt; 10 %</option>
          </select>
        </label>
        <label>{t("hiddenDistributions")} <input inputmode="decimal" bind:value={profit.hidden_distributions} /></label>
        <label>{t("hiddenContributions")} <input inputmode="decimal" bind:value={profit.hidden_contributions} /></label>
        <label>{t("group")}
          <select bind:value={profit.group}>
            <option value={null}>{t("noGroup")}</option>
            <option value="member">{t("groupMember")}</option>
            <option value="parent">{t("groupParent")}</option>
          </select>
        </label>
        {#if profit.group === "member"}<label>{t("groupCompensation")} <input inputmode="decimal" bind:value={profit.group_compensation} /></label>{/if}
        {#if profit.group === "parent"}
          <label>{t("groupIncome")} <input inputmode="decimal" bind:value={profit.group_income} /></label>
          <label>{t("groupTradeIncome")} <input inputmode="decimal" bind:value={profit.group_trade_income} /></label>
        {/if}
        <label>{t("interestCarryforward")} <input inputmode="decimal" bind:value={profit.interest_carryforward} /></label>
        <label>{t("ebitdaCarryforward")} <input inputmode="decimal" bind:value={profit.ebitda_carryforward} /></label>
      </div>
      <label class="check"><input type="checkbox" bind:checked={profit.interest_exception} /> {t("interestException")}</label>
      <div class="row-actions"><button type="submit">{t("keep")}</button></div>
    </form>
  {/if}

  {#if income}
    <form class="panel" onsubmit={saveIncome}>
      <h2>{t("incomeTaxFacts")}</h2>
      <div class="grid">
        <label>{t("otherIncome")} <input inputmode="decimal" bind:value={income.other_income} /></label>
        <label>{t("deductions")} <input inputmode="decimal" bind:value={income.deductions} /></label>
        <label>{t("losses")} <input inputmode="decimal" bind:value={income.losses} /></label>
        <label>{t("churchRate")} <input type="number" min="0" max="10" bind:value={income.church_rate} /></label>
        <label>{t("childAllowances")} <input type="number" min="0" bind:value={income.child_allowances} /></label>
        <label>{t("childBenefit")} <input inputmode="decimal" bind:value={income.child_benefit} /></label>
        <label>{t("singleParentChildren")} <input type="number" min="0" bind:value={income.single_parent_children} /></label>
        <label>{t("extraordinary")} <input inputmode="decimal" bind:value={income.extraordinary} /></label>
        <label>{t("partnershipIncome")} <input inputmode="decimal" bind:value={income.partnership_income} /></label>
        <label>{t("partnershipTradeBase")} <input inputmode="decimal" bind:value={income.partnership_trade_base} /></label>
        <label>{t("partnershipTradeTax")} <input inputmode="decimal" bind:value={income.partnership_trade_tax} /></label>
        <label>{t("capitalIncome")} <input inputmode="decimal" bind:value={income.capital_income} /></label>
      </div>
      <label class="check"><input type="checkbox" bind:checked={income.joint} /> {t("joint")}</label>
      <div class="row-actions"><button type="submit">{t("keep")}</button></div>
    </form>
  {/if}

  <h2>{t("partners")}</h2>
  {#if partners.length === 0}
    <p class="muted">{t("noPartners")}</p>
  {:else}
    <table>
      <tbody>
        {#each partners as p (p.id)}
          <tr>
            <td>{p.name}</td>
            {#if p.accounts}
              <td class="muted">{t("accountsOpen")}</td><td></td>
            {:else}
              <td>
                <select bind:value={liability[p.id]}>
                  <option value="unlimited">{t("unlimited")}</option>
                  <option value="limited">{t("limited")}</option>
                </select>
              </td>
              <td><button class="quiet" onclick={() => open(p.id)}>{t("openAccounts")}</button></td>
            {/if}
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>

<style>
  .check { flex-direction: row; align-items: center; gap: 0.4rem; margin-block: 0.5rem; }
</style>
