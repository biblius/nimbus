import { EditorView, basicSetup } from "codemirror";
import { Compartment, EditorState } from "@codemirror/state";
import type { ViewUpdate } from "@codemirror/view";

const editableConfig = new Compartment();
const lineWrapConfig = new Compartment();
const updateListener = new Compartment();

/** Theme tuned for the Nimbus file preview stage. */
const nimbusTheme = EditorView.theme(
  {
    "&": {
      height: "100%",
      fontSize: "0.875rem",
      backgroundColor: "transparent",
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
      backgroundColor: "transparent",
      border: "none",
      color: "rgb(161 161 170)",
    },
    ".cm-activeLineGutter": {
      backgroundColor: "transparent",
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
  onUpdate?: (update: ViewUpdate) => void;
};

export function init({
  parent,
  doc = "",
  editable = false,
  lineWrap = true,
  onUpdate,
}: InitOptions): EditorView {
  return new EditorView({
    parent,
    state: EditorState.create({
      doc,
      extensions: [
        basicSetup,
        nimbusTheme,
        editableConfig.of(EditorView.editable.of(editable)),
        lineWrapConfig.of(lineWrap ? [EditorView.lineWrapping] : []),
        updateListener.of(
          EditorView.updateListener.of(onUpdate ?? (() => {})),
        ),
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
  view.dispatch({
    effects: lineWrapConfig.reconfigure(
      lineWrap ? [EditorView.lineWrapping] : [],
    ),
  });
}

export function setUpdateHandler(
  view: EditorView,
  fn: (update: ViewUpdate) => void,
) {
  view.dispatch({
    effects: updateListener.reconfigure(EditorView.updateListener.of(fn)),
  });
}
