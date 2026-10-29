<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Choosing an account by its number or a part of its name: a combobox after the pattern of WAI-ARIA. Names that start
     with what was typed come first, then accounts of income and expense, which are what one books on. -->
<script lang="ts">
  import type { Schemas } from "./api";
  import { t } from "./state.svelte";

  type Account = Schemas["AccountView"];
  let {
    accounts,
    value = $bindable(""),
    label,
    onchosen = () => {},
  }: { accounts: Account[]; value?: string; label: string; onchosen?: (account: Account) => void } = $props();

  const id = `account-${Math.random().toString(36).slice(2, 8)}`;
  let open = $state(false);
  let active = $state(0);
  let typed = $state(value);

  const chosen = $derived(accounts.find((a) => a.number === value || a.id === value));
  const matches = $derived.by(() => {
    const text = typed.trim().toLowerCase();
    if (!text || (chosen && (chosen.number === typed || chosen.id === typed))) return [];
    const numeric = /^\d+$/.test(text);
    const found = accounts.filter((a) => (numeric ? (a.number ?? "").startsWith(text) : a.name.toLowerCase().includes(text)));
    if (!numeric) {
      const rank = (a: Account) =>
        (a.name.toLowerCase().startsWith(text) ? 0 : 2) + (a.kind === "income" || a.kind === "expense" ? 0 : 1);
      found.sort((a, b) => rank(a) - rank(b));
    }
    return found.slice(0, 8);
  });

  function take(account: Account) {
    value = account.number ?? account.id;
    typed = value;
    open = false;
    onchosen(account);
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === "ArrowDown") {
      open = true;
      active = Math.min(active + 1, matches.length - 1);
      event.preventDefault();
    } else if (event.key === "ArrowUp") {
      active = Math.max(active - 1, 0);
      event.preventDefault();
    } else if (event.key === "Enter") {
      // Enter takes the account offered; without one chosen, it does not send the form either.
      const match = open ? matches[active] : undefined;
      if (match) take(match);
      if (match || !chosen) event.preventDefault();
    } else if (event.key === "Escape") {
      open = false;
    }
  }
</script>

<label>
  {label}
  <div class="picker">
    <input
      role="combobox"
      aria-label={label}
      aria-expanded={open && matches.length > 0}
      aria-controls={`${id}-list`}
      aria-autocomplete="list"
      aria-activedescendant={open && matches[active] ? `${id}-${active}` : undefined}
      placeholder={t("searchAccount")}
      autocomplete="off"
      bind:value={typed}
      oninput={() => {
        value = typed;
        open = true;
        active = 0;
      }}
      onkeydown={keydown}
      onblur={() => setTimeout(() => (open = false), 150)}
    />
    {#if chosen}<span class="name">{chosen.name}</span>{/if}
    {#if open && matches.length > 0}
      <ul id={`${id}-list`} role="listbox">
        {#each matches as account, i (account.id)}
          <li
            id={`${id}-${i}`}
            role="option"
            aria-selected={i === active}
            onmousedown={(event) => {
              event.preventDefault();
              take(account);
            }}
          >
            <span class="mono">{account.number ?? ""}</span> {account.name}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</label>

<style>
  .picker { position: relative; display: flex; align-items: center; gap: 0.75rem; }
  .picker input { width: 9rem; }
  .name { color: var(--text); font-size: 0.95rem; }
  ul {
    position: absolute; top: 100%; left: 0; z-index: 10; margin: 0.25rem 0 0; padding: 0.25rem 0; list-style: none;
    min-width: 24rem; max-width: 90vw; background: var(--surface); border: 1px solid var(--line); border-radius: 6px;
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.18);
  }
  li { padding: 0.35rem 0.75rem; cursor: pointer; color: var(--text); }
  li[aria-selected="true"] { background: var(--accent); color: var(--on-accent); }
  .mono { display: inline-block; min-width: 3.5rem; }
</style>
