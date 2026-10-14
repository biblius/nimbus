import { EditorView, basicSetup } from "codemirror";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import { lineNumbers, type ViewUpdate } from "@codemirror/view";
import { Vim, vim } from "@replit/codemirror-vim";
import { kv } from "./storage";

const editableConfig = new Compartment();
const lineWrapConfig = new Compartment();
const vimConfig = new Compartment();
const updateListener = new Compartment();

const vimExtension = vim();
const relativeLines = lineNumbersRelative();

/** Persisted editor prefs (shared across text previews). */
export const editorPrefs = $state({
  vim: kv.editorVim.get(),
  lineWrap: kv.editorWrap.get(),
});

type VimExHandlers = {
  /** `:wq` */
  writeQuit: () => void;
  /** `:q` */
  quit: () => void;
  /** `:w` alone — nudge toward `:wq` */
  writeHint: () => void;
};

let vimExHandlers: VimExHandlers | null = null;
let vimExRegistered = false;

function ensureVimExCommands() {
  if (vimExRegistered) {
    return;
  }
  vimExRegistered = true;

  Vim.defineEx("write", "w", () => {
    vimExHandlers?.writeHint();
  });
  Vim.defineEx("quit", "q", () => {
    vimExHandlers?.quit();
  });
  Vim.defineEx("wq", "wq", () => {
    vimExHandlers?.writeQuit();
  });
}

/** Bind Vim ex commands to the active text preview (cleared on unmount). */
export function setVimExHandlers(handlers: VimExHandlers | null) {
  ensureVimExCommands();
  vimExHandlers = handlers;
}

function vimExt(enabled: boolean): Extension[] {
  return enabled ? [vimExtension, relativeLines] : [];
}

/**
 * Relative gutter for VIM motions (same approach as rquest).
 * Stacks with `basicSetup` absolute numbers while VIM is on.
 */
function lineNumbersRelative(): Extension {
  return lineNumbers({ formatNumber: relativeLineNumbers });
}

function relativeLineNumbers(lineNo: number, state: EditorState) {
  const cursorLine = state.doc.lineAt(
    state.selection.asSingle().ranges[0].to,
  ).number;

  if (lineNo === cursorLine) {
    return lineNo.toString();
  }

  return Math.abs(cursorLine - lineNo).toString();
}

/** Theme tuned for the Nimbus file preview stage. */
const nimbusTheme = EditorView.theme(
  {
    "&": {
      height: "100%",
      fontSize: "0.875rem",
      backgroundColor: "rgb(24 24 27)",
      color: "rgb(244 244 245)",
    },
    "&.cm-focused": {
      outline: "none",
    },
    ".cm-scroller": {
      fontFamily:
        'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
      lineHeight: "1.625",
      overflow: "auto",
    },
    ".cm-content": {
      padding: "1rem 1.25rem",
      caretColor: "rgb(244 244 245)",
      minHeight: "100%",
    },
    ".cm-cursor, .cm-dropCursor": {
      borderLeftColor: "rgb(244 244 245)",
    },
    ".cm-gutters": {
      backgroundColor: "rgb(24 24 27)",
      border: "none",
      borderRight: "1px solid rgb(255 255 255 / 0.08)",
      color: "rgb(113 113 122)",
      minWidth: "3rem",
      zIndex: "2",
    },
    ".cm-gutter.cm-lineNumbers .cm-gutterElement": {
      padding: "0 0.75rem 0 0.5rem",
      minWidth: "2.5rem",
    },
    ".cm-activeLineGutter": {
      backgroundColor: "rgb(39 39 42)",
      color: "rgb(212 212 216)",
    },
    ".cm-activeLine": {
      backgroundColor: "rgb(255 255 255 / 0.04)",
    },
    ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": {
      backgroundColor: "rgb(0 130 201 / 0.35) !important",
    },
    ".cm-selectionMatch": {
      backgroundColor: "rgb(0 130 201 / 0.2)",
    },
  },
  { dark: true },
);

export type InitOptions = {
  parent: HTMLElement;
  doc?: string;
  editable?: boolean;
  lineWrap?: boolean;
  vim?: boolean;
  onUpdate?: (update: ViewUpdate) => void;
};

export function init({
  parent,
  doc = "",
  editable = false,
  lineWrap = editorPrefs.lineWrap,
  vim: vimMode = editorPrefs.vim,
  onUpdate,
}: InitOptions): EditorView {
  return new EditorView({
    parent,
    state: EditorState.create({
      doc,
      extensions: [
        // VIM (+ relative lines) before basicSetup so keymaps win.
        vimConfig.of(vimExt(vimMode)),
        basicSetup,
        nimbusTheme,
        editableConfig.of(EditorView.editable.of(editable)),
        lineWrapConfig.of(lineWrap ? [EditorView.lineWrapping] : []),
        updateListener.of(EditorView.updateListener.of(onUpdate ?? (() => {}))),
      ],
    }),
  });
}

export function getContent(view: EditorView): string {
  return view.state.doc.toString();
}

export function setContent(view: EditorView, content: string) {
  if (content === view.state.doc.toString()) {
    return;
  }

  view.dispatch({
    changes: {
      from: 0,
      to: view.state.doc.length,
      insert: content,
    },
  });
}

export function setEditable(view: EditorView, editable: boolean) {
  view.dispatch({
    effects: editableConfig.reconfigure(EditorView.editable.of(editable)),
  });
}

export function setLineWrap(view: EditorView, lineWrap: boolean) {
  editorPrefs.lineWrap = lineWrap;
  kv.editorWrap.set(lineWrap);

  view.dispatch({
    effects: lineWrapConfig.reconfigure(
      lineWrap ? [EditorView.lineWrapping] : [],
    ),
  });
}

/** Enable/disable VIM on the view. Pass `persist: false` for temporary view-only changes. */
export function setVim(
  view: EditorView,
  enabled: boolean,
  options: { persist?: boolean } = {},
) {
  const persist = options.persist ?? true;

  if (persist) {
    editorPrefs.vim = enabled;
    kv.editorVim.set(enabled);
  }

  view.dispatch({
    effects: vimConfig.reconfigure(vimExt(enabled)),
  });
}
