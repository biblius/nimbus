<script lang="ts">
  import { untrack } from "svelte";
  import { ApiError, ValidationError } from "../../api";
  import { toastError } from "../../toast";
  import type { Root, RootData } from "../../types";
  import Button from "../ui/button/button.svelte";
  import Field from "../ui/field/field.svelte";
  import QuotaField from "./QuotaField.svelte";

  let {
    title,
    description,
    folder,
    save,
    cancel,
  }: {
    title: string;
    description: string;
    folder: Root;
    save: (root: RootData) => Promise<void>;
    cancel?: () => void;
  } = $props();

  let name = $state(untrack(() => folder.name));
  let quotaBytes = $state(untrack(() => folder.quota));
  let quotaError = $state("");
  let quotaValid = $state(true);
  let validation = $state<ValidationError | undefined>();
  let busy = $state(false);

  async function onSubmit(event: SubmitEvent) {
    event.preventDefault();

    validation = undefined;

    if (!quotaValid) {
      return;
    }

    busy = true;

    try {
      await save({
        name,
        quotaBytes,
        members: [],
      });
    } catch (cause) {
      if (cause instanceof ValidationError) {
        validation = cause;
        return;
      }
      toastError(
        cause instanceof TypeError
          ? "The server could not be reached"
          : cause instanceof ApiError && cause.status === 403
            ? cause.message
            : "Could not save the folder",
      );
    } finally {
      busy = false;
    }
  }
</script>

<form class="space-y-5" onsubmit={onSubmit}>
  <div class="flex items-end justify-between gap-3 border-b pb-3">
    <div class="min-w-0">
      <h3 class="text-sm font-semibold tracking-tight">{title}</h3>
      <p class="mt-0.5 text-xs text-muted-foreground">{description}</p>
    </div>
    <div class="flex shrink-0 items-center gap-1.5">
      {#if cancel}
        <Button
          type="button"
          variant="ghost"
          size="sm"
          disabled={busy}
          onclick={cancel}
        >
          Cancel
        </Button>
      {/if}
      <Button type="submit" size="sm" disabled={busy}>Save</Button>
    </div>
  </div>

  <section class="space-y-3">
    <p
      class="text-xs font-medium uppercase tracking-wide text-muted-foreground"
    >
      Details
    </p>

    <Field
      label="Name"
      id="folder-name"
      dense
      bind:value={name}
      error={validation?.field("name")}
      required
    />

    <QuotaField
      bind:bytes={quotaBytes}
      bind:error={quotaError}
      bind:valid={quotaValid}
      serverError={validation?.field("quota_bytes")}
    />
  </section>
</form>
