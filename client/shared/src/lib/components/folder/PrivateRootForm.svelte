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

<form class="overflow-hidden rounded-xl border bg-card" onsubmit={onSubmit}>
  <div class="border-b px-5 py-4">
    <h3 class="text-sm font-semibold tracking-tight">{title}</h3>
    <p class="mt-1 text-sm text-muted-foreground">{description}</p>
  </div>

  <div class="space-y-4 px-5 py-4">
    <Field
      label="Name"
      id="folder-name"
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
  </div>

  <div
    class="flex items-center justify-end gap-2 border-t bg-muted/30 px-5 py-3"
  >
    {#if cancel}
      <Button type="button" variant="outline" onclick={cancel}>Cancel</Button>
    {/if}
    <Button type="submit" disabled={busy}>Save</Button>
  </div>
</form>
