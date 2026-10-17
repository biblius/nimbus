<script lang="ts">
  import { File as FileIcon, Folder } from "@lucide/svelte";
  import { CFB } from "xlsx";
  import { formatBytes } from "../../../format";

  let {
    url,
    name,
    credentials = "omit",
  }: {
    url: string;
    name: string;
    /** Use `"include"` for authenticated blob routes. */
    credentials?: RequestCredentials;
  } = $props();

  type ZipEntry = {
    path: string;
    name: string;
    size: number;
    isDirectory: boolean;
  };

  let loading = $state(true);
  let error = $state("");
  let entries = $state.raw<ZipEntry[]>([]);

  const itemLabel = $derived(
    entries.length === 1 ? "1 item" : `${entries.length} items`,
  );

  function basename(path: string): string {
    const slash = path.lastIndexOf("/");
    return slash >= 0 ? path.slice(slash + 1) : path;
  }

  /** List zip members via SheetJS CFB (https://git.sheetjs.com/sheetjs/js-cfb). */
  function parseArchive(data: Uint8Array): ZipEntry[] {
    const cfb = CFB.read(data, { type: "array" });

    const root: string = cfb.FullPaths[0] ?? "";

    const parsed: ZipEntry[] = [];

    // Skip FullPaths[0] (root storage). Same walk as js-cfb itself.
    for (let i = 1; i < cfb.FullPaths.length; i++) {
      const fp: string = cfb.FullPaths[i].slice(root.length);

      // CFB seed entry injected by SheetJS — not a real zip member
      if (fp === "\u0001Sh33tJ5") {
        continue;
      }

      const isDirectory = fp.endsWith("/");
      const path = isDirectory ? fp.slice(0, -1) : fp;

      if (!path) {
        continue;
      }

      parsed.push({
        path,
        name: basename(path),
        size: cfb.FileIndex[i]?.size ?? 0,
        isDirectory,
      });
    }

    parsed.sort((a, b) => {
      if (a.isDirectory !== b.isDirectory) {
        return a.isDirectory ? -1 : 1;
      }
      return a.path.localeCompare(b.path, undefined, { sensitivity: "base" });
    });

    return parsed;
  }

  $effect(() => {
    const contentUrl = url;
    const creds = credentials;

    loading = true;
    error = "";
    entries = [];

    let cancelled = false;

    fetch(contentUrl, { credentials: creds })
      .then((res) => {
        if (!res.ok) {
          throw new Error("fetch failed");
        }
        return res.arrayBuffer();
      })
      .then((buffer) => {
        if (cancelled) {
          return;
        }

        entries = parseArchive(new Uint8Array(buffer));
        loading = false;
      })
      .catch(() => {
        if (cancelled) {
          return;
        }
        error = "Could not load archive";
        loading = false;
      });

    return () => {
      cancelled = true;
    };
  });
</script>

<div class="flex h-full w-full flex-col gap-3">
  {#if loading}
    <div
      class="m-auto size-8 animate-spin rounded-full border-2 border-white/20 border-t-white/80"
      aria-label="Loading"
    ></div>
  {:else if error}
    <p class="m-auto text-sm text-red-300">{error}</p>
  {:else}
    <div class="flex min-h-0 flex-1 flex-col gap-3">
      <div
        class="flex shrink-0 items-center rounded-xl bg-zinc-900/90 px-3 py-2 ring-1 ring-white/10 backdrop-blur-md"
      >
        <p class="text-xs tracking-wide text-white/50">{itemLabel}</p>
      </div>

      <div
        class="min-h-0 flex-1 overflow-auto rounded-xl bg-zinc-900/90 ring-1 ring-white/10"
        aria-label={name}
      >
        {#if entries.length === 0}
          <p class="m-auto p-6 text-center text-sm text-white/50">
            This archive is empty
          </p>
        {:else}
          <ul class="divide-y divide-white/5 py-1" role="list">
            {#each entries as entry (entry.path)}
              <li
                class="flex items-center gap-3 px-3 py-2 text-sm text-zinc-200"
              >
                {#if entry.isDirectory}
                  <Folder
                    class="size-4 shrink-0 text-amber-300/80"
                    strokeWidth={1.75}
                    aria-hidden="true"
                  />
                {:else}
                  <FileIcon
                    class="size-4 shrink-0 text-white/50"
                    strokeWidth={1.75}
                    aria-hidden="true"
                  />
                {/if}
                <span class="min-w-0 flex-1 truncate" title={entry.path}>
                  {entry.path}{entry.isDirectory ? "/" : ""}
                </span>
                <span
                  class="shrink-0 text-xs text-white/40 tabular-nums"
                  aria-label={entry.isDirectory
                    ? "Directory"
                    : formatBytes(entry.size)}
                >
                  {entry.isDirectory ? "—" : formatBytes(entry.size)}
                </span>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </div>
  {/if}
</div>
