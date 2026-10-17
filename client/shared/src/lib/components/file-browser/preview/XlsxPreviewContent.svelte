<script lang="ts">
  import { read, utils, type WorkBook } from "xlsx";
  import { cn } from "../../../utils";

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

  let loading = $state(true);
  let error = $state("");
  let workbook = $state.raw<WorkBook | null>(null);
  let sheetNames = $state<string[]>([]);
  let activeSheet = $state("");
  let fileData = $state.raw<Uint8Array | null>(null);

  const rows = $derived.by(() => {
    if (!workbook || !activeSheet) {
      return [] as unknown[][];
    }

    const sheet = workbook.Sheets[activeSheet];

    if (!sheet) {
      return [] as unknown[][];
    }

    return utils.sheet_to_json(sheet, {
      header: 1,
      defval: "",
    }) as unknown[][];
  });

  const colCount = $derived(
    rows.reduce((max, row) => Math.max(max, row.length), 0),
  );

  function parseSheet(data: Uint8Array, sheetName: string): WorkBook {
    return read(data, { type: "array", sheets: sheetName });
  }

  $effect(() => {
    const contentUrl = url;
    const creds = credentials;

    loading = true;
    error = "";
    workbook = null;
    sheetNames = [];
    activeSheet = "";
    fileData = null;

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

        const data = new Uint8Array(buffer);
        const meta = read(data, { type: "array", bookSheets: true });
        const names = meta.SheetNames;
        const first = names[0] ?? "";

        if (cancelled) {
          return;
        }

        fileData = data;
        sheetNames = names;
        activeSheet = first;
        workbook = first ? parseSheet(data, first) : null;
        loading = false;
      })
      .catch(() => {
        if (cancelled) {
          return;
        }
        error = "Could not load spreadsheet";
        loading = false;
      });

    return () => {
      cancelled = true;
    };
  });

  function selectSheet(sheetName: string) {
    if (sheetName === activeSheet || !fileData) {
      return;
    }

    activeSheet = sheetName;
    workbook = parseSheet(fileData, sheetName);
  }

  function cellText(cell: unknown): string {
    if (cell == null) {
      return "";
    }
    return String(cell);
  }
</script>

<div class="flex h-full w-full flex-col gap-3">
  {#if loading}
    <div
      class="m-auto size-8 animate-spin rounded-full border-2 border-white/20 border-t-white/80"
      aria-label="Loading"
    ></div>
  {:else if error}
    <p class="m-auto text-sm text-red-300">{error}</p>
  {:else if workbook}
    <div class="flex min-h-0 flex-1 flex-col gap-3">
      {#if sheetNames.length > 1}
        <div
          class="flex shrink-0 gap-1.5 overflow-x-auto rounded-xl bg-zinc-900/90 p-2 ring-1 ring-white/10 backdrop-blur-md"
          role="tablist"
          aria-label="Sheets"
        >
          {#each sheetNames as sheetName (sheetName)}
            <button
              type="button"
              role="tab"
              class={cn(
                "shrink-0 rounded-full px-3 py-1.5 text-xs font-medium tracking-wide whitespace-nowrap transition-colors",
                activeSheet === sheetName
                  ? "bg-white/14 text-white ring-1 ring-white/40"
                  : "bg-white/10 text-white/70 ring-1 ring-transparent hover:bg-white/14 hover:text-white",
              )}
              aria-selected={activeSheet === sheetName}
              onclick={() => selectSheet(sheetName)}
            >
              {sheetName}
            </button>
          {/each}
        </div>
      {/if}

      <div
        class="min-h-0 flex-1 overflow-auto rounded-xl bg-zinc-900/90 ring-1 ring-white/10"
        aria-label={name}
      >
        {#if rows.length === 0}
          <p class="m-auto p-6 text-center text-sm text-white/50">
            This sheet is empty
          </p>
        {:else}
          <table
            class="w-max min-w-full border-collapse text-left text-sm text-zinc-800"
          >
            <tbody>
              {#each rows as row, rowIndex (rowIndex)}
                <tr
                  class={rowIndex === 0
                    ? "sticky top-0 z-10 bg-zinc-100 shadow-sm"
                    : rowIndex % 2 === 0
                      ? "bg-white"
                      : "bg-zinc-50"}
                >
                  {#each Array.from({ length: colCount }, (_, i) => i) as colIndex (colIndex)}
                    {@const text = cellText(row[colIndex])}
                    {#if rowIndex === 0}
                      <th
                        class="border border-zinc-200 px-3 py-2 text-left text-xs font-semibold whitespace-nowrap text-zinc-700"
                      >
                        {text}
                      </th>
                    {:else}
                      <td
                        class="border border-zinc-200 px-3 py-1.5 whitespace-nowrap text-zinc-800"
                      >
                        {text}
                      </td>
                    {/if}
                  {/each}
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>
    </div>
  {/if}
</div>
