<script lang="ts">
  import { Check, MailWarning } from "@lucide/svelte";
  import {
    ApiError,
    ValidationError,
    type ValidationField,
  } from "../../api-errors";
  import { formatBytes } from "../../format";
  import { toastError, toastSuccess } from "../../toast";
  import type { NimbusConfigUpdate, NimbusConfiguration } from "../../types";
  import Banner from "../ui/banner/banner.svelte";
  import Button from "../ui/button/button.svelte";
  import Field from "../ui/field/field.svelte";
  import Input from "../ui/input/input.svelte";
  import Label from "../ui/label/label.svelte";

  const GIB = 1024 ** 3;
  const REDACTED = "<redacted>";

  let {
    config = $bindable(),
    save,
  }: {
    config: NimbusConfiguration;
    save: (input: NimbusConfigUpdate) => Promise<NimbusConfiguration>;
  } = $props();

  let nimbusOrigin = $state(config.nimbusOrigin);
  let fileRetentionDays = $state(String(config.fileRetentionDays));
  let uploadMaxGib = $state(bytesToGibInput(config.uploadMaxBytes));
  let emailFrom = $state(config.emailFrom);
  let smtpHost = $state(config.smtpHost);
  let smtpPort = $state(String(config.smtpPort));
  let smtpUsername = $state(config.smtpUsername);
  let smtpPassword = $state("");
  let clearPassword = $state(false);
  let passwordWasSet = $state(config.smtpPassword === REDACTED);

  let busy = $state(false);
  let validation = $state<ValidationError | undefined>();
  let localErrors = $state<Record<string, ValidationField | undefined>>({});

  const uploadBytesPreview = $derived(gibInputToBytes(uploadMaxGib));

  const smtpIncomplete = $derived(
    !smtpHost.trim() ||
      !smtpUsername.trim() ||
      clearPassword ||
      (!passwordWasSet && !smtpPassword.trim()),
  );

  const passwordError = $derived(fieldError("smtpPassword", "smtp_password"));

  const dirty = $derived(
    nimbusOrigin.trim() !== config.nimbusOrigin ||
      Number(fileRetentionDays) !== config.fileRetentionDays ||
      gibInputToBytes(uploadMaxGib) !== config.uploadMaxBytes ||
      emailFrom !== config.emailFrom ||
      smtpHost !== config.smtpHost ||
      Number(smtpPort) !== config.smtpPort ||
      smtpUsername !== config.smtpUsername ||
      smtpPassword.length > 0 ||
      clearPassword,
  );

  function isHttpUrl(value: string): boolean {
    try {
      const url = new URL(value);
      return url.protocol === "http:" || url.protocol === "https:";
    } catch {
      return false;
    }
  }

  function bytesToGibInput(bytes: number): string {
    const gib = bytes / GIB;
    if (!Number.isFinite(gib) || gib <= 0) {
      return "";
    }
    // Enough precision to round-trip byte values (incl. sub-GiB limits).
    return gib.toFixed(12).replace(/\.?0+$/, "");
  }

  function gibInputToBytes(value: string): number {
    const n = Number(value);
    if (!Number.isFinite(n) || n <= 0) {
      return 0;
    }
    return Math.round(n * GIB);
  }

  function syncFrom(next: NimbusConfiguration) {
    config = next;
    nimbusOrigin = next.nimbusOrigin;
    fileRetentionDays = String(next.fileRetentionDays);
    uploadMaxGib = bytesToGibInput(next.uploadMaxBytes);
    emailFrom = next.emailFrom;
    smtpHost = next.smtpHost;
    smtpPort = String(next.smtpPort);
    smtpUsername = next.smtpUsername;
    smtpPassword = "";
    clearPassword = false;
    passwordWasSet = next.smtpPassword === REDACTED;
    validation = undefined;
    localErrors = {};
  }

  function fieldError(
    camel: string,
    snake: string,
  ): ValidationField | undefined {
    return (
      localErrors[camel] ?? validation?.field(camel) ?? validation?.field(snake)
    );
  }

  function requiredField(field: string, message: string): ValidationField {
    return {
      field,
      code: "required",
      params: {},
      message,
      location: `/${field}`,
    };
  }

  function validateClient(): boolean {
    const next: Record<string, ValidationField | undefined> = {};
    const origin = nimbusOrigin.trim();

    if (!origin) {
      next.nimbusOrigin = requiredField(
        "nimbusOrigin",
        "Nimbus origin is required",
      );
    } else if (!isHttpUrl(origin)) {
      next.nimbusOrigin = {
        field: "nimbusOrigin",
        code: "url",
        params: {},
        message: "Enter a valid URL (e.g. https://files.example.com)",
        location: "/nimbusOrigin",
      };
    }

    const days = Number(fileRetentionDays);
    if (!Number.isInteger(days) || days < 1) {
      next.fileRetentionDays = {
        field: "fileRetentionDays",
        code: "range",
        params: {},
        message: "Retention must be at least 1 day",
        location: "/fileRetentionDays",
      };
    }

    const gib = Number(uploadMaxGib);
    if (!Number.isFinite(gib) || gib <= 0) {
      next.uploadMaxBytes = {
        field: "uploadMaxBytes",
        code: "range",
        params: {},
        message: "Upload max must be greater than 0 GiB",
        location: "/uploadMaxBytes",
      };
    }

    const port = Number(smtpPort);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      next.smtpPort = {
        field: "smtpPort",
        code: "range",
        params: {},
        message: "SMTP port must be between 1 and 65535",
        location: "/smtpPort",
      };
    }

    localErrors = next;
    return Object.keys(next).length === 0;
  }

  function buildPatch(): NimbusConfigUpdate {
    const patch: NimbusConfigUpdate = {};

    const origin = nimbusOrigin.trim();
    if (origin !== config.nimbusOrigin) {
      patch.nimbusOrigin = origin;
    }

    const days = Number(fileRetentionDays);
    if (days !== config.fileRetentionDays) {
      patch.fileRetentionDays = days;
    }

    const bytes = gibInputToBytes(uploadMaxGib);
    if (bytes !== config.uploadMaxBytes) {
      patch.uploadMaxBytes = bytes;
    }

    if (emailFrom !== config.emailFrom) {
      patch.emailFrom = emailFrom;
    }
    if (smtpHost !== config.smtpHost) {
      patch.smtpHost = smtpHost;
    }

    const port = Number(smtpPort);
    if (port !== config.smtpPort) {
      patch.smtpPort = port;
    }

    if (smtpUsername !== config.smtpUsername) {
      patch.smtpUsername = smtpUsername;
    }

    if (smtpPassword.length > 0) {
      patch.smtpPassword = smtpPassword;
    } else if (clearPassword) {
      patch.smtpPassword = "";
    }

    return patch;
  }

  function reset() {
    syncFrom(config);
  }

  function markClearPassword() {
    smtpPassword = "";
    clearPassword = true;
  }

  function undoClearPassword() {
    clearPassword = false;
  }

  async function onSubmit(event: SubmitEvent) {
    event.preventDefault();

    if (busy || !dirty) {
      return;
    }

    validation = undefined;

    if (!validateClient()) {
      return;
    }

    const patch = buildPatch();
    if (Object.keys(patch).length === 0) {
      return;
    }

    busy = true;

    try {
      const updated = await save(patch);
      syncFrom(updated);
      toastSuccess("Settings saved");
    } catch (cause) {
      if (cause instanceof ValidationError) {
        validation = cause;
      } else {
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : cause instanceof ApiError
              ? cause.message
              : "Could not save settings",
        );
      }
    } finally {
      busy = false;
    }
  }
</script>

<form class="space-y-5" onsubmit={onSubmit}>
  <div
    class="sticky top-0 z-10 -mx-1 flex flex-wrap items-end justify-between gap-3 border-b bg-background/95 px-1 py-3 backdrop-blur-sm"
  >
    <div class="min-w-0 space-y-0.5">
      <h2 class="text-lg font-semibold tracking-tight">Settings</h2>
      <p class="text-sm text-muted-foreground">
        These values override environment defaults and are saved to
        <span class="font-mono text-[11px]">config.json</span>.
      </p>
    </div>
    <div class="flex shrink-0 items-center gap-1.5">
      <Button
        type="button"
        variant="ghost"
        size="sm"
        disabled={busy || !dirty}
        onclick={reset}
      >
        Cancel
      </Button>
      <Button type="submit" size="sm" disabled={busy || !dirty}>Save</Button>
    </div>
  </div>

  {#if smtpIncomplete}
    <Banner tone="info">
      {#snippet icon()}<MailWarning />{/snippet}
      Outbound email is disabled until SMTP host, username, and password are configured.
    </Banner>
  {/if}

  <section class="space-y-4 rounded-lg border bg-card p-4 sm:p-5">
    <div>
      <h3 class="text-sm font-semibold tracking-tight">General</h3>
    </div>

    <div class="space-y-3">
      <p
        class="text-xs font-medium tracking-wide text-muted-foreground uppercase"
      >
        Instance
      </p>

      <Field
        label="Nimbus origin"
        id="config-nimbus-origin"
        dense
        bind:value={nimbusOrigin}
        error={fieldError("nimbusOrigin", "nimbus_origin")}
        placeholder="https://files.example.com"
        autocomplete="off"
        required
      />
      <p class="-mt-1 text-xs text-muted-foreground">
        Base URL used for links, WebAuthn, and email. Changing this will
        invalidate any sent links or passkeys, change it only when necessary.
      </p>

      <Field
        label="File retention (days)"
        id="config-file-retention"
        type="number"
        inputmode="numeric"
        dense
        bind:value={fileRetentionDays}
        error={fieldError("fileRetentionDays", "file_retention_days")}
        min={1}
        step={1}
        required
      />
      <p class="-mt-1 text-xs text-muted-foreground">
        How long files are kept in trash.
      </p>

      <Field
        label="Upload max (GiB)"
        id="config-upload-max"
        type="number"
        inputmode="decimal"
        dense
        bind:value={uploadMaxGib}
        error={fieldError("uploadMaxBytes", "upload_max_bytes")}
        min={0.001}
        step="any"
        required
      />
      <p class="-mt-1 text-xs text-muted-foreground">
        {#if uploadBytesPreview > 0}
          ≈ {formatBytes(uploadBytesPreview)} ({uploadBytesPreview.toLocaleString()}
          bytes)
        {:else}
          Maximum size of a single upload.
        {/if}
      </p>
    </div>
  </section>

  <section class="space-y-4 rounded-lg border bg-card p-4 sm:p-5">
    <div>
      <h3 class="text-sm font-semibold tracking-tight">Email</h3>
      <p class="mt-0.5 text-xs text-muted-foreground">
        SMTP for password reset and notification mail.
      </p>
    </div>

    <div class="space-y-3">
      <p
        class="text-xs font-medium tracking-wide text-muted-foreground uppercase"
      >
        Sender
      </p>

      <Field
        label="From name"
        id="config-email-from"
        dense
        bind:value={emailFrom}
        error={fieldError("emailFrom", "email_from")}
        autocomplete="off"
      />
    </div>

    <div class="space-y-3 border-t pt-4">
      <p
        class="text-xs font-medium tracking-wide text-muted-foreground uppercase"
      >
        SMTP
      </p>

      <div class="grid gap-3 sm:grid-cols-3">
        <div class="sm:col-span-2">
          <Field
            label="Host"
            id="config-smtp-host"
            dense
            bind:value={smtpHost}
            error={fieldError("smtpHost", "smtp_host")}
            placeholder="smtp.example.com"
            autocomplete="off"
          />
        </div>
        <Field
          label="Port"
          id="config-smtp-port"
          type="number"
          inputmode="numeric"
          dense
          bind:value={smtpPort}
          error={fieldError("smtpPort", "smtp_port")}
          min={1}
          max={65535}
          step={1}
        />
      </div>

      <Field
        label="Username"
        id="config-smtp-username"
        dense
        bind:value={smtpUsername}
        error={fieldError("smtpUsername", "smtp_username")}
        autocomplete="off"
      />

      <div class="space-y-1">
        <div class="flex items-center gap-2">
          <Label
            for="config-smtp-password"
            class="text-xs text-muted-foreground"
          >
            Password
          </Label>
          {#if passwordWasSet && !clearPassword}
            <Check
              size={14}
              strokeWidth={2}
              class="text-muted-foreground"
              aria-label="Password configured"
            />
          {/if}
        </div>
        <div class="flex items-center gap-1.5">
          <Input
            id="config-smtp-password"
            type="password"
            class="h-8 min-w-0 flex-1 shadow-none"
            bind:value={smtpPassword}
            placeholder={clearPassword ? "Cleared on save" : "SMTP password"}
            autocomplete="new-password"
            disabled={clearPassword}
            aria-invalid={passwordError ? true : undefined}
            oninput={() => {
              if (smtpPassword.length > 0) {
                clearPassword = false;
              }
            }}
          />
          {#if passwordWasSet && !clearPassword}
            <Button
              type="button"
              variant="outline"
              size="sm"
              class="shrink-0"
              disabled={busy}
              onclick={markClearPassword}
            >
              Clear
            </Button>
          {:else if clearPassword}
            <Button
              type="button"
              variant="ghost"
              size="sm"
              class="shrink-0"
              disabled={busy}
              onclick={undoClearPassword}
            >
              Undo
            </Button>
          {/if}
        </div>
        {#if passwordError?.message || passwordError?.code}
          <p class="text-xs text-destructive">
            {passwordError.message || passwordError.code}
          </p>
        {/if}
      </div>
    </div>
  </section>
</form>
