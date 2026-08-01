import { basicSetup } from "codemirror";
import { Decoration, EditorView } from "@codemirror/view";
import { Compartment, EditorState, RangeSetBuilder, StateEffect, StateField } from "@codemirror/state";
import { setDiagnostics } from "@codemirror/lint";
import { catppuccinLatte, catppuccinMocha } from "@catppuccin/codemirror";
import { Pane } from "tweakpane";
import init, { analyze, compile } from "../pkg/chi.js";

const STORAGE_KEY = "chi-playground-source";
const DEFAULT_EXAMPLE = "o";

const exampleSelect = document.querySelector("#example");
const glyphSelect = document.querySelector("#glyph");
const debugCheckbox = document.querySelector("#debug");
const paramsPanel = document.querySelector("#params");
const inspector = document.querySelector("#inspector");
const inspectorGroups = inspector.querySelector(".groups");
const preview = document.querySelector("#preview");
const status = document.querySelector("#status");

let overrides = {};
let paramsSignature = "";
let renderTimer = null;
let pane = null;
let paneState = {};
// pane.refresh() re-emits change events; this flag keeps those echoes from
// being recorded as user overrides.
let paneSyncing = false;

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

// Editor theme follows the OS: page colors flip via the CSS media query, but
// a CodeMirror extension is fixed at construction, so it lives in a
// Compartment and is reconfigured when the preference changes.
const darkScheme = window.matchMedia("(prefers-color-scheme: dark)");
const editorTheme = new Compartment();

const view = new EditorView({
  parent: document.querySelector("#editor"),
  extensions: [
    basicSetup,
    editorTheme.of(darkScheme.matches ? catppuccinMocha : catppuccinLatte),
    // chi uses `#` line comments. Without a Language there is no languageData,
    // so toggleComment (Ctrl+/) has no comment token to work with; supply it.
    EditorState.languageData.of(() => [{ commentTokens: { line: "#" } }]),
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

darkScheme.addEventListener("change", (event) => {
  view.dispatch({
    effects: editorTheme.reconfigure(event.matches ? catppuccinMocha : catppuccinLatte),
  });
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
    updateInspector(result.measures);
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
    // Keep the pane; just sync displayed values from the compile result.
    paneSyncing = true;
    for (const param of params) paneState[param.name] = param.value;
    pane?.refresh();
    paneSyncing = false;
    return;
  }
  paramsSignature = signature;
  pane?.dispose();
  pane = null;
  paneState = {};
  if (params.length === 0) return;

  pane = new Pane({ container: paramsPanel, title: "params" });
  for (const param of params) {
    paneState[param.name] = param.value;
    pane
      .addBinding(paneState, param.name, {
        min: param.min,
        max: param.max,
        step: (param.max - param.min) / 200,
      })
      .on("change", (event) => {
        if (paneSyncing) return;
        overrides[param.name] = event.value;
        scheduleRender();
      });
  }
  pane.addButton({ title: "reset" }).on("click", () => {
    overrides = {};
    paramsSignature = "";
    render();
  });
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

// Every named number in the document: metrics, top-level bindings, and the
// glyph's own. Derived measures are otherwise write-only — the source can
// compute a value but nothing shows it — so this is where a probe like
// `bulge = distance(point(main, 0.2), center) - r` becomes readable.
function updateInspector(measures) {
  const wanted = glyphSelect.value;
  const groups = [
    ["metrics", measures.metrics],
    ["bindings", measures.bindings],
    ...measures.glyphs.filter(([name]) => !wanted || name === wanted),
  ].filter(([, numbers]) => numbers.length > 0);

  inspector.hidden = groups.length === 0;
  inspectorGroups.innerHTML = "";
  for (const [title, numbers] of groups) {
    const group = document.createElement("div");
    group.className = "group";

    const label = document.createElement("span");
    label.className = "label";
    label.textContent = title;
    group.append(label);

    for (const [name, value] of numbers) {
      const measure = document.createElement("span");
      measure.className = "measure";
      const key = document.createElement("span");
      key.textContent = name;
      const shown = document.createElement("span");
      shown.className = "value";
      shown.textContent = precise(value);
      measure.append(key, shown);
      group.append(measure);
    }
    inspectorGroups.append(group);
  }
}

// Four decimals is not enough for a slider label but is right for a probe:
// the fitter's radial error on a 290-unit circle is 0.0787 units.
function precise(value) {
  if (Number.isInteger(value)) return String(value);
  const text = value.toFixed(4).replace(/0+$/, "").replace(/\.$/, "");
  return text === "-0" ? "0" : text;
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
