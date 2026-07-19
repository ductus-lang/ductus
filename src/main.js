import { basicSetup } from "codemirror";
import { Decoration, EditorView } from "@codemirror/view";
import { RangeSetBuilder, StateEffect, StateField } from "@codemirror/state";
import { setDiagnostics } from "@codemirror/lint";
import init, { analyze, compile } from "../pkg/chi.js";

const STORAGE_KEY = "chi-playground-source";
const DEFAULT_EXAMPLE = "o";

const exampleSelect = document.querySelector("#example");
const glyphSelect = document.querySelector("#glyph");
const debugCheckbox = document.querySelector("#debug");
const paramsPanel = document.querySelector("#params");
const preview = document.querySelector("#preview");
const status = document.querySelector("#status");

let overrides = {};
let paramsSignature = "";
let renderTimer = null;

// Syntax colors live in a decoration field so they follow edits (deco.map)
// between analyses and are replaced wholesale when fresh spans arrive.
const setHighlights = StateEffect.define();

const highlightField = StateField.define({
  create() {
    return Decoration.none;
  },
  update(deco, tr) {
    deco = deco.map(tr.changes);
    for (const effect of tr.effects) {
      if (effect.is(setHighlights)) deco = effect.value;
    }
    return deco;
  },
  provide: (field) => EditorView.decorations.from(field),
});

function buildDecorations(highlights) {
  const builder = new RangeSetBuilder();
  for (const { from, to, class: cls } of highlights) {
    if (to > from) builder.add(from, to, Decoration.mark({ class: `chi-${cls}` }));
  }
  return builder.finish();
}

const view = new EditorView({
  parent: document.querySelector("#editor"),
  extensions: [
    basicSetup,
    highlightField,
    EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        localStorage.setItem(STORAGE_KEY, update.state.doc.toString());
        // Defer past the current update: dispatching mid-update is not allowed.
        queueMicrotask(onDocChange);
      }
    }),
  ],
});

function scheduleRender() {
  clearTimeout(renderTimer);
  renderTimer = setTimeout(render, 200);
}

// Cheap parse-only pass on every keystroke: repaint colors and, if the syntax
// is broken, own the diagnostics; otherwise hand off to the debounced compile.
function onDocChange() {
  const errors = paintSyntax();
  if (errors.length === 0) scheduleRender();
}

// Run analyze(); repaint highlights and syntax diagnostics. Returns the syntax
// errors so callers can decide whether to proceed to a full compile.
function paintSyntax() {
  const source = view.state.doc.toString();
  const { highlights, errors } = JSON.parse(analyze(source));
  const diagnostics = errors.map((error) => ({
    from: error.from,
    to: error.to > error.from ? error.to : Math.min(error.from + 1, view.state.doc.length),
    severity: "error",
    message: error.message,
  }));
  view.dispatch(setDiagnostics(view.state, diagnostics), {
    effects: setHighlights.of(buildDecorations(highlights)),
  });

  if (errors.length > 0) {
    status.hidden = false;
    const extra = errors.length > 1 ? ` (+${errors.length - 1} more)` : "";
    const first = errors[0];
    status.textContent = `line ${first.line}:${first.column} — ${first.message}${extra}`;
  }
  return errors;
}

function render() {
  // Syntax errors own the diagnostics; a broken parse never reaches compile.
  if (paintSyntax().length > 0) return;

  const source = view.state.doc.toString();
  const result = JSON.parse(
    compile(source, JSON.stringify(overrides), glyphSelect.value, debugCheckbox.checked),
  );

  if (result.ok) {
    status.hidden = true;
    preview.innerHTML = result.svg;
    updateParams(result.params);
    updateGlyphs(result.glyphs);
    view.dispatch(setDiagnostics(view.state, []));
  } else {
    status.hidden = false;
    const { line, column, message } = result.error;
    status.textContent = line > 0 ? `line ${line}:${column} — ${message}` : message;
    markError(result.error);
  }
}

function markError({ line, column, message }) {
  if (line < 1 || line > view.state.doc.lines) {
    view.dispatch(setDiagnostics(view.state, []));
    return;
  }
  const lineInfo = view.state.doc.line(line);
  const from = Math.min(lineInfo.from + column - 1, lineInfo.to);
  view.dispatch(
    setDiagnostics(view.state, [
      { from, to: lineInfo.to, severity: "error", message },
    ]),
  );
}

function updateParams(params) {
  const signature = params.map((p) => `${p.name}:${p.min}:${p.max}`).join(";");
  if (signature === paramsSignature) {
    // Keep the sliders; just refresh displayed values.
    for (const param of params) {
      const value = paramsPanel.querySelector(`[data-param="${param.name}"] .value`);
      if (value) value.textContent = format(param.value);
    }
    return;
  }
  paramsSignature = signature;
  paramsPanel.innerHTML = "";
  for (const param of params) {
    const row = document.createElement("div");
    row.className = "param";
    row.dataset.param = param.name;

    const label = document.createElement("span");
    label.textContent = param.name;

    const slider = document.createElement("input");
    slider.type = "range";
    slider.min = param.min;
    slider.max = param.max;
    slider.step = (param.max - param.min) / 200;
    slider.value = param.value;
    slider.addEventListener("input", () => {
      overrides[param.name] = Number(slider.value);
      value.textContent = format(Number(slider.value));
      scheduleRender();
    });

    const value = document.createElement("span");
    value.className = "value";
    value.textContent = format(param.value);

    row.append(label, slider, value);
    paramsPanel.append(row);
  }
  if (params.length > 0) {
    const reset = document.createElement("button");
    reset.className = "reset";
    reset.textContent = "reset";
    reset.addEventListener("click", () => {
      overrides = {};
      paramsSignature = "";
      render();
    });
    paramsPanel.append(reset);
  }
}

function updateGlyphs(glyphs) {
  const current = glyphSelect.value;
  const options = ["", ...glyphs];
  const existing = [...glyphSelect.options].map((option) => option.value);
  if (options.join(",") !== existing.join(",")) {
    glyphSelect.innerHTML = "";
    for (const name of options) {
      const option = document.createElement("option");
      option.value = name;
      option.textContent = name === "" ? "all" : name;
      glyphSelect.append(option);
    }
    glyphSelect.value = options.includes(current) ? current : "";
  }
}

function format(value) {
  return Number.isInteger(value) ? String(value) : value.toFixed(2);
}

async function loadExample(name) {
  const response = await fetch(`examples/${name}.chi`);
  const source = await response.text();
  overrides = {};
  paramsSignature = "";
  view.dispatch({
    changes: { from: 0, to: view.state.doc.length, insert: source },
  });
}

exampleSelect.addEventListener("change", () => loadExample(exampleSelect.value));
glyphSelect.addEventListener("change", render);
debugCheckbox.addEventListener("change", render);

await init({ module_or_path: new URL("./pkg/chi_bg.wasm", document.baseURI) });

const saved = localStorage.getItem(STORAGE_KEY);
if (saved) {
  view.dispatch({ changes: { from: 0, to: 0, insert: saved } });
  render();
} else {
  exampleSelect.value = DEFAULT_EXAMPLE;
  await loadExample(DEFAULT_EXAMPLE);
}
