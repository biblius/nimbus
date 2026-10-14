<script lang="ts">
  import { untrack } from "svelte";
  import type { ValidationField } from "../../api";
  import Label from "../ui/label/label.svelte";

  const UNITS = [
    { id: "MB", size: 1024 ** 2 },
    { id: "GB", size: 1024 ** 3 },
    { id: "TB", size: 1024 ** 4 },
  ] as const;

  type Unit = (typeof UNITS)[number]["id"] | "unlimited";

  let {
    bytes = $bindable(0),
    error = $bindable(""),
    valid = $bindable(true),
    serverError,
    id = "folder-quota",
  }: {
    /** Quota in bytes. `0` is unlimited. */
    bytes?: number;
    error?: string;
    valid?: boolean;
    serverError?: ValidationField;
    id?: string;
  } = $props();

  function split(value: number): { amount: string; unit: Unit } {
    if (value <= 0) {
      return { amount: "", unit: "unlimited" };
    }

    let chosen: (typeof UNITS)[number] = UNITS[0];

    for (const unit of UNITS) {
      if (value >= unit.size) {
        chosen = unit;
      }
    }

    const rounded = Math.round((value / chosen.size) * 100) / 100;
    const amount = Number.isInteger(rounded)
      ? String(rounded)
      : rounded.toFixed(2).replace(/\.?0+$/, "");

    return { amount, unit: chosen.id };
  }

  const initial = untrack(() => split(bytes));
  let amount = $state(initial.amount);
  let unit = $state<Unit>(initial.unit);

  const serverMessage = $derived(
    serverError?.message || serverError?.code || "",
  );
  const invalid = $derived(Boolean(error || serverMessage));

  function commit() {
    if (unit === "unlimited") {
      error = "";
      valid = true;
      bytes = 0;
      return;
    }

    const text = amount.trim();

    if (!/^\d+(\.\d+)?$/.test(text)) {
      error = "Enter a size";
      valid = false;
      return;
    }

    const size = UNITS.find((item) => item.id === unit)!.size;
    const next = Math.round(Number(text) * size);

    if (!Number.isSafeInteger(next)) {
      error = "That quota is too large";
      valid = false;
      return;
    }

    error = "";
    valid = true;
    bytes = next;
  }
</script>

<div class="space-y-2">
  <Label for={id}>Quota</Label>
  <div
    class={[
      "flex h-9 overflow-hidden rounded-md border bg-field shadow-sm focus-within:ring-2",
      invalid
        ? "border-destructive focus-within:ring-destructive"
        : "border-input focus-within:ring-ring",
    ]}
  >
    <input
      {id}
      class="min-w-0 flex-1 bg-transparent px-3 text-sm tabular-nums outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:text-muted-foreground"
      type="text"
      inputmode="decimal"
      autocomplete="off"
      placeholder={unit === "unlimited" ? "No cap" : "0"}
      disabled={unit === "unlimited"}
      aria-invalid={invalid ? true : undefined}
      value={unit === "unlimited" ? "" : amount}
      oninput={(event) => {
        amount = event.currentTarget.value;
        commit();
      }}
    />
    <select
      class="w-28 shrink-0 border-l border-input bg-muted/40 px-2 text-sm outline-none"
      aria-label="Quota unit"
      bind:value={unit}
      onchange={commit}
    >
      <option value="MB">MB</option>
      <option value="GB">GB</option>
      <option value="TB">TB</option>
      <option value="unlimited">Unlimited</option>
    </select>
  </div>
  {#if error}
    <p class="text-sm text-destructive">{error}</p>
  {:else if serverMessage}
    <p class="text-sm text-destructive">{serverMessage}</p>
  {:else if unit === "unlimited"}
    <p class="text-xs text-muted-foreground">No storage cap.</p>
  {/if}
</div>
