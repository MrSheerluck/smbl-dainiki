import { syntaxTree } from "@codemirror/language";
import {
  Decoration,
  MatchDecorator,
  ViewPlugin,
  WidgetType,
} from "@codemirror/view";

const HEADING_CLASSES = {
  ATXHeading1: "cm-heading-1",
  ATXHeading2: "cm-heading-2",
  ATXHeading3: "cm-heading-3",
  ATXHeading4: "cm-heading-4",
  ATXHeading5: "cm-heading-5",
  ATXHeading6: "cm-heading-6",
};

class BulletWidget extends WidgetType {
  eq() {
    return true;
  }

  toDOM() {
    const bullet = document.createElement("span");
    bullet.className = "cm-bullet";
    bullet.textContent = "•";
    return bullet;
  }
}

class CheckboxWidget extends WidgetType {
  constructor(checked) {
    super();
    this.checked = checked;
  }

  eq(other) {
    return other.checked === this.checked;
  }

  toDOM() {
    const box = document.createElement("input");
    box.type = "checkbox";
    box.checked = this.checked;
    box.disabled = true;
    box.className = "cm-task-checkbox";
    return box;
  }
}

function activeLines(state) {
  const lines = new Set();

  for (const range of state.selection.ranges) {
    const start = state.doc.lineAt(range.from).number;
    const end = state.doc.lineAt(range.to).number;

    for (let line = start; line <= end; line += 1) {
      lines.add(line);
    }
  }

  return lines;
}

function buildDecorations(view) {
  const { state } = view;
  const doc = state.doc;
  const active = activeLines(state);
  const decorations = [];
  const seenLines = new Set();

  const addLine = (line, className) => {
    const key = `${line.from}:${className}`;
    if (seenLines.has(key)) return;

    seenLines.add(key);
    decorations.push(Decoration.line({ class: className }).range(line.from));
  };

  const addMark = (from, to, className) => {
    if (from < to) {
      decorations.push(Decoration.mark({ class: className }).range(from, to));
    }
  };

  const hide = (from, to) => {
    if (from < to) {
      decorations.push(Decoration.replace({}).range(from, to));
    }
  };

  const hideWithSpace = (from, to) => {
    const line = doc.lineAt(from);
    let end = to;

    while (end < line.to && /\s/.test(doc.sliceString(end, end + 1))) {
      end += 1;
    }

    hide(from, end);
  };

  const addBlockLines = (node, className) => {
    const start = doc.lineAt(node.from);
    const end = doc.lineAt(Math.max(node.from, node.to - 1));

    for (let number = start.number; number <= end.number; number += 1) {
      addLine(doc.line(number), className);
    }
  };

  const markInner = (node, className) => {
    const first = node.node.firstChild;
    const last = node.node.lastChild;

    if (first && last && last.from > first.to) {
      addMark(first.to, last.from, className);
    } else {
      addMark(node.from, node.to, className);
    }
  };

  for (const { from, to } of view.visibleRanges) {
    syntaxTree(state).iterate({
      from,
      to,
      enter: (node) => {
        const name = node.name;
        const line = doc.lineAt(node.from);
        const isActive = active.has(line.number);

        if (HEADING_CLASSES[name]) {
          addLine(line, HEADING_CLASSES[name]);
          return;
        }

        if (name === "Blockquote") {
          addBlockLines(node, "cm-blockquote");
          return;
        }

        if (name === "FencedCode") {
          addBlockLines(node, "cm-code-block");
          return;
        }

        if (name === "StrongEmphasis") {
          markInner(node, "cm-strong");
          return;
        }

        if (name === "Emphasis") {
          markInner(node, "cm-em");
          return;
        }

        if (name === "Strikethrough") {
          markInner(node, "cm-strike");
          return;
        }

        if (name === "InlineCode") {
          markInner(node, "cm-inline-code");
          return;
        }

        if (name === "Link") {
          const marks = [];

          for (let child = node.node.firstChild; child; child = child.nextSibling) {
            if (child.name === "LinkMark") marks.push(child);
          }

          if (marks.length >= 2) {
            addMark(marks[0].to, marks[1].from, "cm-link");
          }

          return;
        }

        if (name === "HorizontalRule") {
          if (!isActive) {
            addLine(line, "cm-hr");
            hide(node.from, node.to);
          }
          return;
        }

        if (name === "HeaderMark" && !isActive) {
          hideWithSpace(node.from, node.to);
          return;
        }

        if (name === "QuoteMark" && !isActive) {
          hideWithSpace(node.from, node.to);
          return;
        }

        if ((name === "EmphasisMark" || name === "StrikethroughMark") && !isActive) {
          hide(node.from, node.to);
          return;
        }

        if (name === "CodeMark") {
          if (node.node.parent?.name === "InlineCode" && !isActive) {
            hide(node.from, node.to);
          }
          return;
        }

        if (name === "LinkMark" || name === "URL") {
          if (node.node.parent?.name === "Link" && !isActive) {
            hide(node.from, node.to);
          }
          return;
        }

        if (name === "ListMark") {
          const text = doc.sliceString(node.from, node.to);

          if (/^\d/.test(text)) {
            addMark(node.from, node.to, "cm-list-number");
          } else if (!isActive) {
            decorations.push(
              Decoration.replace({ widget: new BulletWidget() }).range(
                node.from,
                node.to
              )
            );
          }
          return;
        }

        if (name === "TaskMarker" && !isActive) {
          const text = doc.sliceString(node.from, node.to).toLowerCase();
          decorations.push(
            Decoration.replace({
              widget: new CheckboxWidget(text.includes("x")),
            }).range(node.from, node.to)
          );
        }
      },
    });
  }

  return Decoration.set(decorations, true);
}

const livePreviewPlugin = ViewPlugin.fromClass(
  class {
    constructor(view) {
      this.decorations = buildDecorations(view);
    }

    update(update) {
      if (update.docChanged || update.selectionSet || update.viewportChanged) {
        this.decorations = buildDecorations(update.view);
      }
    }
  },
  {
    decorations: (plugin) => plugin.decorations,
  }
);

const tagMatcher = new MatchDecorator({
  regexp: /(?<=^|[\s([{,;])#[^\s#]+/gu,
  decoration: Decoration.mark({ class: "cm-tag" }),
});

const tagHighlighter = ViewPlugin.fromClass(
  class {
    constructor(view) {
      this.decorations = tagMatcher.createDeco(view);
    }

    update(update) {
      this.decorations = tagMatcher.updateDeco(update, this.decorations);
    }
  },
  {
    decorations: (plugin) => plugin.decorations,
  }
);

export const livePreview = [livePreviewPlugin, tagHighlighter];
