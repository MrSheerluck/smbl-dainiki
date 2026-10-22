import { EditorState } from "@codemirror/state";
import {
  EditorView,
  drawSelection,
  dropCursor,
  keymap,
  placeholder,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown, markdownLanguage } from "@codemirror/lang-markdown";
import { livePreview } from "./live-preview.js";

const theme = EditorView.theme({
  "&": {
    color: "var(--color-on-surface)",
    backgroundColor: "transparent",
    fontSize: "0.95rem",
  },
  ".cm-scroller": {
    fontFamily: "var(--font-journal)",
    lineHeight: "1.625rem",
    minHeight: "22rem",
    overflow: "auto",
  },
  ".cm-content": {
    padding: "0",
    caretColor: "var(--color-primary)",
  },
  ".cm-line": {
    padding: "0",
  },
  "&.cm-focused": {
    outline: "none",
  },
  ".cm-cursor, .cm-dropCursor": {
    borderLeftColor: "var(--color-primary)",
    borderLeftWidth: "1.5px",
  },
  ".cm-selectionBackground, ::selection": {
    backgroundColor: "var(--color-surface-container-high)",
  },
  ".cm-activeLine": {
    backgroundColor: "transparent",
  },
  ".cm-placeholder": {
    color:
      "color-mix(in oklab, var(--color-on-surface-variant) 45%, transparent)",
  },
  ".cm-heading-1": {
    fontFamily: "var(--font-display)",
    fontSize: "2rem",
    lineHeight: "2.5rem",
  },
  ".cm-heading-2": {
    fontFamily: "var(--font-display)",
    fontSize: "1.5rem",
    lineHeight: "2rem",
  },
  ".cm-heading-3": {
    fontFamily: "var(--font-display)",
    fontSize: "1.25rem",
    lineHeight: "1.75rem",
  },
  ".cm-heading-4, .cm-heading-5, .cm-heading-6": {
    fontFamily: "var(--font-display)",
    fontSize: "1.05rem",
    lineHeight: "1.625rem",
  },
  ".cm-strong": {
    fontWeight: "600",
  },
  ".cm-em": {
    fontStyle: "italic",
  },
  ".cm-strike": {
    textDecoration: "line-through",
  },
  ".cm-inline-code": {
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
    fontSize: "0.86em",
    backgroundColor: "var(--color-surface-container)",
    borderRadius: "0.125rem",
    padding: "0.08rem 0.25rem",
  },
  ".cm-link": {
    color: "var(--color-primary)",
    textDecoration: "underline",
    textUnderlineOffset: "2px",
  },
  ".cm-tag": {
    color: "var(--color-secondary)",
  },
  ".cm-blockquote": {
    borderLeft:
      "2px solid color-mix(in oklab, var(--color-secondary) 45%, transparent)",
    paddingLeft: "1rem",
    color: "var(--color-on-surface-variant)",
  },
  ".cm-bullet": {
    color: "var(--color-secondary)",
    fontFamily: "var(--font-ui)",
  },
  ".cm-list-number": {
    color: "var(--color-secondary)",
    fontFamily: "var(--font-ui)",
  },
  ".cm-hr": {
    borderBottom: "1px solid var(--color-outline-variant)",
  },
  ".cm-task-checkbox": {
    accentColor: "var(--color-primary)",
    marginRight: "0.35rem",
  },
  ".cm-code-block": {
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
    fontSize: "0.85rem",
    backgroundColor: "var(--color-surface-container-low)",
  },
});

export function createEditor(parent, onChange) {
  const state = EditorState.create({
    doc: "",
    extensions: [
      history(),
      drawSelection(),
      dropCursor(),
      EditorView.lineWrapping,
      markdown({ base: markdownLanguage }),
      livePreview,
      theme,
      placeholder("Write an entry…"),
      keymap.of([...defaultKeymap, ...historyKeymap]),
      EditorView.contentAttributes.of({
        spellcheck: "true",
        "aria-label": "Journal entry",
      }),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          onChange();
        }
      }),
    ],
  });

  return new EditorView({ state, parent });
}

export function setEditorText(view, text) {
  view.dispatch({
    changes: {
      from: 0,
      to: view.state.doc.length,
      insert: text,
    },
  });
}

export function getEditorText(view) {
  return view.state.doc.toString();
}
