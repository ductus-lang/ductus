//! WebAssembly bindings for the playground: one call from source to SVG.

use serde::Serialize;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
#[serde(untagged)]
enum Response {
    Ok {
        ok: bool,
        svg: String,
        params: Vec<ParamInfo>,
        glyphs: Vec<String>,
    },
    Err {
        ok: bool,
        error: ErrorInfo,
    },
}

#[derive(Serialize)]
struct ParamInfo {
    name: String,
    value: f64,
    default: f64,
    min: f64,
    max: f64,
}

#[derive(Serialize)]
struct ErrorInfo {
    /// 1-based; 0 when the error has no source position.
    line: usize,
    column: usize,
    message: String,
}

/// Compile chi source to SVG.
///
/// `overrides_json`: JSON object of param overrides, e.g. `{"weight": 700}`.
/// `glyph`: render only this glyph (empty string = all).
/// Returns a JSON string: `{ok, svg, params, glyphs}` or `{ok, error}`.
#[wasm_bindgen]
pub fn compile(source: &str, overrides_json: &str, glyph: &str, debug: bool) -> String {
    console_error_panic_hook::set_once();
    let response = match compile_inner(source, overrides_json, glyph, debug) {
        Ok(response) => response,
        Err(error) => Response::Err { ok: false, error },
    };
    serde_json::to_string(&response).expect("response serializes")
}

fn compile_inner(
    source: &str,
    overrides_json: &str,
    glyph: &str,
    debug: bool,
) -> Result<Response, ErrorInfo> {
    let overrides: HashMap<String, f64> = if overrides_json.trim().is_empty() {
        HashMap::new()
    } else {
        serde_json::from_str(overrides_json).map_err(|error| ErrorInfo {
            line: 0,
            column: 0,
            message: format!("invalid overrides: {error}"),
        })?
    };

    let tree = chi_core::parse::parse(source).map_err(|error| ErrorInfo {
        line: error.line,
        column: error.column,
        message: error.message,
    })?;

    let program = chi_core::lower::lower(&tree, source).map_err(|error| ErrorInfo {
        line: error.span.line,
        column: error.span.column,
        message: error.message,
    })?;

    let document = chi_core::eval::evaluate(&program, &overrides).map_err(|error| ErrorInfo {
        line: error.span.line,
        column: error.span.column,
        message: error.message,
    })?;

    let options = chi_core::render::RenderOptions {
        glyph: (!glyph.is_empty()).then(|| glyph.to_string()),
        debug,
    };
    let svg = chi_core::render::render(&document, &options).map_err(|error| ErrorInfo {
        line: 0,
        column: 0,
        message: error.message,
    })?;

    Ok(Response::Ok {
        ok: true,
        svg,
        params: document
            .params
            .iter()
            .map(|param| ParamInfo {
                name: param.name.clone(),
                value: param.value,
                default: param.default,
                min: param.min,
                max: param.max,
            })
            .collect(),
        glyphs: document
            .glyphs
            .iter()
            .map(|glyph| glyph.name.clone())
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_a_glyph() {
        let response = compile(
            "metrics { upm = 1000; asc = 750; desc = -220; }\nglyph l { advance = 100; contour c = (0,0) -- (50,0) -- (50,700) -- (0,700) -- cycle; }\n",
            "",
            "",
            false,
        );
        assert!(response.contains("\"ok\":true"), "{response}");
        assert!(response.contains("<svg"));
    }

    #[test]
    fn reports_errors_with_position() {
        let response = compile("x = undefined_thing;\n", "", "", false);
        assert!(response.contains("\"ok\":false"), "{response}");
        assert!(response.contains("\"line\":1"));
    }

    #[test]
    fn applies_overrides() {
        let source = "params { w = 100 in 0..200; }\nglyph g { advance = w; contour c = (0,0) -- (w,0) -- (w,10) -- cycle; }\n";
        let default = compile(source, "", "", false);
        let wide = compile(source, "{\"w\": 200}", "", false);
        assert!(default.contains("\"ok\":true"));
        assert!(wide.contains("\"ok\":true"));
        assert_ne!(default, wide);
    }
}
