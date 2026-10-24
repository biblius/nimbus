<script lang="ts">
  import {
    ArrowLeft,
    FilePenLine,
    FilePlusCorner,
    FolderInput,
    RotateCcwClock,
    Pencil,
    RotateCcw,
    Share2,
    Trash,
  } from "@lucide/svelte";
  import type { Component } from "svelte";
  import { formatDatetime } from "../format";
  import { navigate, p } from "../router";
  import { session } from "../session.svelte";
  import { toastError } from "../toast";
  import type { Activity, ActivityData, ActivityTarget, Root } from "../types";
  import Button from "./ui/button/button.svelte";
  import InfiniteScroller from "./ui/infinite-list/infinite-scroller.svelte";

  let { root }: { root: Root } = $props();

  let activities = $state.raw<Activity[]>([]);
  let total = $state<number | null>(null);
  let perPage = $state(20);

  let page = 1;
  let done = false;
  let pending: Promise<boolean> | null = null;
  let generation = 0;

  const ROW_COLS = "grid-cols-[1.5rem_minmax(0,1fr)_7.5rem]";

  type ActivityCopy = {
    verb: string;
    items: string;
    /** e.g. `to Documents` or rename old name. */
    path: string | null;
    type: ActivityData["type"];
    fileId: string | null;
  };

  function fileIdFor(data: ActivityData): string | null {
    switch (data.type) {
      case "create":
      case "trash":
      case "share":
        return data.t_type === "single" ? data.id : null;
      case "move":
      case "restore":
        return data.target.t_type === "single" ? data.target.id : null;
      case "rename":
      case "edit":
        return data.id;
    }
  }

  function formatActivityTime(iso: string): string {
    const date = new Date(iso);

    if (Number.isNaN(date.getTime())) {
      return iso;
    }

    const diffSec = Math.round((date.getTime() - Date.now()) / 1000);
    const abs = Math.abs(diffSec);
    const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

    if (abs < 60) {
      return rtf.format(diffSec, "second");
    }

    const diffMin = Math.round(diffSec / 60);

    if (Math.abs(diffMin) < 60) {
      return rtf.format(diffMin, "minute");
    }

    const diffHr = Math.round(diffMin / 60);

    if (Math.abs(diffHr) < 24) {
      return rtf.format(diffHr, "hour");
    }

    const diffDay = Math.round(diffHr / 24);

    if (Math.abs(diffDay) < 7) {
      return rtf.format(diffDay, "day");
    }

    return formatDatetime(iso);
  }

  function pathLabel(path: string): string {
    return path === "" ? "Root" : path;
  }

  function targetItems(target: ActivityTarget): string {
    return target.t_type === "single" ? target.name : `${target.count} files`;
  }

  function verbFor(type: ActivityData["type"]): string {
    switch (type) {
      case "create":
        return "created";
      case "move":
        return "moved";
      case "rename":
        return "renamed";
      case "trash":
        return "trashed";
      case "restore":
        return "restored";
      case "share":
        return "shared";
      case "edit":
        return "edited";
    }
  }

  function summarize(activity: Activity): ActivityCopy {
    const { data } = activity;
    const fileId = fileIdFor(data);

    switch (data.type) {
      case "create":
      case "trash":
      case "share":
        return {
          verb: verbFor(data.type),
          items: targetItems(data),
          path: null,
          type: data.type,
          fileId,
        };
      case "move":
        return {
          verb: verbFor(data.type),
          items: targetItems(data.target),
          path: `to ${pathLabel(data.to)}`,
          type: data.type,
          fileId,
        };
      case "restore":
        return {
          verb: verbFor(data.type),
          items: targetItems(data.target),
          path: `to ${pathLabel(data.to)}`,
          type: data.type,
          fileId,
        };
      case "rename":
        return {
          verb: verbFor(data.type),
          items: data.to,
          path: `from ${data.from}`,
          type: data.type,
          fileId,
        };
      case "edit":
        return {
          verb: verbFor(data.type),
          items: data.name,
          path: null,
          type: data.type,
          fileId,
        };
    }
  }

  function iconFor(type: ActivityData["type"]): Component {
    switch (type) {
      case "create":
        return FilePlusCorner;
      case "move":
        return FolderInput;
      case "rename":
        return Pencil;
      case "trash":
        return Trash;
      case "restore":
        return RotateCcw;
      case "share":
        return Share2;
      case "edit":
        return FilePenLine;
    }
  }

  /** Shared color for the type icon and verb. */
  function toneFor(type: ActivityData["type"]): string {
    switch (type) {
      case "create":
        return "text-emerald-600 dark:text-emerald-400";
      case "move":
        return "text-sky-600 dark:text-sky-400";
      case "rename":
        return "text-amber-600 dark:text-amber-400";
      case "trash":
        return "text-destructive";
      case "restore":
        return "text-teal-600 dark:text-teal-400";
      case "share":
        return "text-primary";
      case "edit":
        return "text-violet-600 dark:text-violet-400";
    }
  }

  async function loadNext(): Promise<boolean> {
    if (done) {
      return false;
    }

    if (pending) {
      return pending;
    }

    const current = page;
    const currentGeneration = generation;
    const rootId = root.id;

    const task = session.client
      .listActivities(rootId, { page: current, perPage })
      .then((listed) => {
        if (currentGeneration !== generation) {
          return false;
        }

        activities = [...activities, ...listed.items];
        total = listed.total;
        page = current + 1;
        done = listed.items.length === 0 || activities.length >= listed.total;

        return !done;
      })
      .catch((cause) => {
        if (currentGeneration !== generation) {
          return false;
        }

        done = true;
        toastError(
          cause instanceof TypeError
            ? "The server could not be reached"
            : "Could not load activity",
        );
        return false;
      })
      .finally(() => {
        if (pending === task) {
          pending = null;
        }
      });

    pending = task;

    return pending;
  }
</script>

<div class="relative flex min-h-0 min-w-0 flex-1 flex-col gap-3">
  <div class="flex min-w-0 shrink-0 items-center gap-2 px-0.5">
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      class="shrink-0 text-muted-foreground hover:text-foreground"
      aria-label="Back to files"
      title="Back to files"
      onclick={() =>
        navigate("/files/:root_id", { params: { root_id: root.id } })}
    >
      <ArrowLeft size={18} />
    </Button>
    <div class="flex min-w-0 items-baseline gap-2">
      <h1 class="text-sm font-medium text-foreground">Activity</h1>
      <span class="truncate text-sm text-muted-foreground" title={root.name}>
        {root.name}
      </span>
    </div>
  </div>

  <div
    class="@container flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-lg border bg-card"
  >
    <div
      class="grid h-10 shrink-0 {ROW_COLS} items-center gap-x-3 border-b px-3 text-xs font-medium text-muted-foreground"
    >
      <span aria-hidden="true"></span>
      <span>Action</span>
      <span class="text-right">When</span>
    </div>

    <InfiniteScroller
      {loadNext}
      bind:loadAmount={perPage}
      class="h-full max-h-none min-h-0 flex-1 rounded-none border-0 bg-transparent shadow-none"
    >
      {#if total === 0}
        <div
          class="flex h-full min-h-48 flex-col items-center justify-center gap-2 px-6 py-16 text-center"
        >
          <RotateCcwClock
            class="size-10 text-muted-foreground opacity-40"
            strokeWidth={1.5}
          />
          <p class="text-sm text-muted-foreground">No recent activity</p>
        </div>
      {:else if activities.length > 0}
        <ul class="divide-y">
          {#each activities as activity, i (`${activity.createdAt}-${i}`)}
            {@const copy = summarize(activity)}
            {@const Icon = iconFor(copy.type)}
            {@const tone = toneFor(copy.type)}
            <li class="px-3 py-2 transition-colors hover:bg-accent/60">
              <div class="grid {ROW_COLS} items-center gap-x-3">
                <Icon
                  size={15}
                  class="justify-self-center {tone}"
                  strokeWidth={1.75}
                  aria-hidden="true"
                />
                <div class="min-w-0">
                  <p class="truncate text-sm leading-5">
                    <span class="font-medium text-foreground"
                      >{activity.username}</span
                    >
                    <span class="font-medium {tone}"> {copy.verb}</span>
                    {#if copy.items}
                      {#if copy.fileId}
                        {@const fileId = copy.fileId}
                        <a
                          class="text-foreground underline-offset-2 hover:underline"
                          href={p("/files/:root_id", {
                            params: { root_id: root.id },
                            search: { file: fileId },
                          })}
                          onclick={(event) => {
                            if (
                              event.button !== 0 ||
                              event.metaKey ||
                              event.ctrlKey ||
                              event.shiftKey ||
                              event.altKey
                            ) {
                              return;
                            }

                            event.preventDefault();
                            navigate("/files/:root_id", {
                              params: { root_id: root.id },
                              search: { file: fileId },
                            });
                          }}
                        >
                          {" " + copy.items}
                        </a>
                      {:else}
                        <span class="text-foreground"> {copy.items}</span>
                      {/if}
                    {/if}
                  </p>
                  {#if copy.path}
                    <p
                      class="mt-0.5 truncate text-xs text-muted-foreground"
                      title={copy.path}
                    >
                      {copy.path}
                    </p>
                  {/if}
                </div>
                <time
                  class="text-right text-xs text-muted-foreground tabular-nums"
                  datetime={activity.createdAt}
                  title={formatDatetime(activity.createdAt)}
                >
                  {formatActivityTime(activity.createdAt)}
                </time>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </InfiniteScroller>
  </div>
</div>
