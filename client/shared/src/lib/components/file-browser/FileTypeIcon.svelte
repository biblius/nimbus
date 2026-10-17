<script lang="ts">
  import {
    AppWindow,
    Archive,
    BookOpen,
    File as FileIcon,
    FileCode,
    FileText,
    Film,
    Image,
    Music,
    Type,
  } from "@lucide/svelte";
  import { fileFamily, type FileFamily } from "../../fileKind";
  import type { FileType } from "../../types";
  import { cn } from "../../utils";

  let {
    fileType = null,
    size = 18,
    class: className = "",
    strokeWidth = 1.75,
  }: {
    fileType?: FileType | null;
    size?: number;
    class?: string;
    strokeWidth?: number;
  } = $props();

  const family = $derived(fileFamily(fileType));

  const icons: Record<FileFamily, typeof FileIcon> = {
    app: AppWindow,
    book: BookOpen,
    image: Image,
    video: Film,
    audio: Music,
    font: Type,
    doc: FileText,
    archive: Archive,
    text: FileCode,
    other: FileIcon,
  };

  const Icon = $derived(icons[family]);
</script>

<Icon {size} {strokeWidth} class={cn(className)} />
