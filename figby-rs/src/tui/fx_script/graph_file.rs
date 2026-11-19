//! Saving and loading FX graphs (docs/fx-scripting-plan.md, phase 5). The
//! filesystem side only: the text itself comes from [`graph_to_json`] and
//! [`FxGraph::to_dsl`], so the graph core stays free of `std::fs`.
//!
//! A saved graph is two files in `<config dir>/fx-graphs/`:
//! - `<name>.fxgraph.json`: the IR, which is what loads back into the editor.
//! - `<name>.figfx.toml`: the emitted DSL as a one-effect script (trigger
//!   `manual`), so the graph can be opened as an effect script as well.

use std::io;
use std::path::{Path, PathBuf};

use super::edit::{graph_from_json, graph_to_json};
use super::graph::FxGraph;

/// Extension of the IR file.
pub const GRAPH_EXTENSION: &str = ".fxgraph.json";

/// `<config dir>/fx-graphs`, or `None` when there is no config dir.
pub fn default_dir() -> Option<PathBuf> {
    crate::config::config_dir().map(|d| d.join("fx-graphs"))
}

/// A file stem safe on every platform: ASCII letters, digits, `-` and `_`
/// (anything else becomes `_`); never empty.
pub fn file_stem(name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if stem.is_empty() {
        "graph".into()
    } else {
        stem
    }
}

/// Path of the IR file for `name` in `dir`.
pub fn graph_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{}{GRAPH_EXTENSION}", file_stem(name)))
}

/// Path of the DSL script for `name` in `dir`.
pub fn script_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!(
        "{}{}",
        file_stem(name),
        super::script::SCRIPT_EXTENSION
    ))
}

/// The `.figfx.toml` text for one manual effect named `name`.
pub fn script_text(name: &str, dsl: &str) -> String {
    // Literal multi-line string: the DSL never contains `'''`.
    format!(
        "# Emitted from the FX Graph editor; edit the graph, not this file.\n\
         [[effect]]\nname    = \"{}\"\ntrigger = \"manual\"\nsource  = '''\n{dsl}\n'''\n",
        file_stem(name)
    )
}

/// What a save wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub graph: PathBuf,
    pub script: PathBuf,
}

/// Write `graph` as `<name>.fxgraph.json` and `<name>.figfx.toml` in `dir`,
/// creating `dir` if needed. An invalid graph is refused before anything is
/// written.
pub fn save(dir: &Path, name: &str, graph: &FxGraph) -> io::Result<Saved> {
    let dsl = graph
        .to_dsl()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    std::fs::create_dir_all(dir)?;
    let saved = Saved {
        graph: graph_path(dir, name),
        script: script_path(dir, name),
    };
    std::fs::write(&saved.graph, graph_to_json(graph))?;
    std::fs::write(&saved.script, script_text(name, &dsl))?;
    Ok(saved)
}

/// Read and validate `<name>.fxgraph.json` from `dir`.
pub fn load(dir: &Path, name: &str) -> io::Result<FxGraph> {
    load_path(&graph_path(dir, name))
}

/// Read and validate a graph file.
pub fn load_path(path: &Path) -> io::Result<FxGraph> {
    let text = crate::bounded_io::read_bounded_string(path, crate::bounded_io::MAX_TEXT_BYTES)?;
    graph_from_json(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::fx_script::presets;
    use crate::tui::fx_script::script::parse_script;

    #[test]
    fn stems_are_sanitised() {
        assert_eq!(file_stem("intro"), "intro");
        assert_eq!(file_stem("../a b"), "___a_b");
        assert_eq!(file_stem(""), "graph");
    }

    #[test]
    fn save_then_load_round_trips_and_script_parses() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("fx-graphs");
        for p in presets::all() {
            let saved = save(&sub, p.name, &p.graph).unwrap();
            assert_eq!(load(&sub, p.name).unwrap(), p.graph, "{}", p.name);
            let text = std::fs::read_to_string(&saved.script).unwrap();
            let script = parse_script(&text).unwrap_or_else(|e| panic!("{}: {e:?}", p.name));
            assert_eq!(script.effects.len(), 1, "{}", p.name);
        }
    }

    #[test]
    fn load_reports_missing_and_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load(dir.path(), "nope").unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        std::fs::write(graph_path(dir.path(), "bad"), "{ nope").unwrap();
        assert_eq!(
            load(dir.path(), "bad").unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
