<script lang="ts">
  import { tick, untrack } from "svelte";
  import type { ValidationField } from "../../api-errors";
  import Input from "../ui/input/input.svelte";
  import Label from "../ui/label/label.svelte";
  import Select from "../ui/select/select.svelte";

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
  let touched = $state(false);
  let input = $state<HTMLInputElement>();

  $effect(() => {
    const form = input?.form;

    if (!form) {
      return;
    }

    const onSubmit = () => {
      touched = true;
      commit();
    };

    form.addEventListener("submit", onSubmit);
    return () => form.removeEventListener("submit", onSubmit);
  });

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
      error = touched ? "Enter a size" : "";
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
  <div class="flex">
    <Input
      bind:ref={input}
      {id}
      class="min-w-0 flex-1 rounded-r-none tabular-nums focus-visible:z-10"
      type="text"
      inputmode="decimal"
      autocomplete="off"
      placeholder={unit === "unlimited" ? "No cap" : "0"}
      disabled={unit === "unlimited"}
      aria-invalid={invalid ? true : undefined}
      value={unit === "unlimited" ? "" : amount}
      oninput={(event) => {
        amount = event.currentTarget.value;
        touched = true;
        commit();
      }}
      onblur={() => {
        touched = true;
        commit();
      }}
    />
    <Select
      class="-ml-px w-32 shrink-0 rounded-l-none"
      aria-label="Quota unit"
      aria-invalid={invalid ? true : undefined}
      bind:value={unit}
      onchange={async () => {
        commit();

        if (unit !== "unlimited" && !amount.trim()) {
          await tick();
          input?.focus();
        }
      }}
    >
      <option value="MB">MB</option>
      <option value="GB">GB</option>
      <option value="TB">TB</option>
      <option value="unlimited">Unlimited</option>
    </Select>
  </div>
  {#if error}
    <p class="text-sm text-destructive">{error}</p>
  {:else if serverMessage}
    <p class="text-sm text-destructive">{serverMessage}</p>
  {:else if unit === "unlimited"}
    <p class="text-xs text-muted-foreground">No storage cap.</p>
  {/if}
</div>
