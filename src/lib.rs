//! `johnny_dsl` — 策略 DSL 的引擎。
//!
//! A strategy is a program, so the language it is written in should be one:
//! mruby runs inside this extension, the DSL's vocabulary is defined in
//! Ruby, and exactly one thing crosses back into Python — a JSON IR. Neither
//! side has to bend to the other's syntax, which is what both YAML and a
//! Python-shaped DSL kept costing.

mod engine;
mod sys;

pub use engine::{evaluate as evaluate_source, Failure};

use pyo3::exceptions::{PyOSError, PyValueError};
use pyo3::prelude::*;

/// Evaluate DSL source and return its IR as JSON text.
///
/// Args:
///     source: The strategy source.
///     name: What to call it in a backtrace, its path by convention.
///
/// Returns:
///     The IR, as JSON.
///
/// Raises:
///     ValueError: If the source does not evaluate — the message carries
///         Ruby's own error and backtrace.
#[pyfunction]
#[pyo3(signature = (source, name = "<dsl>"))]
fn evaluate(source: &str, name: &str) -> PyResult<String> {
    engine::evaluate(source, name).map_err(|failure| PyValueError::new_err(failure.0))
}

/// Evaluate a DSL file and return its IR as JSON text.
///
/// Args:
///     path: The file to read.
///
/// Returns:
///     The IR, as JSON.
///
/// Raises:
///     OSError: If the file cannot be read.
///     ValueError: If the source does not evaluate.
#[pyfunction]
fn evaluate_file(path: &str) -> PyResult<String> {
    let source = std::fs::read_to_string(path).map_err(|error| PyOSError::new_err(format!("{path}: {error}")))?;
    evaluate(&source, path)
}

/// The extension module.
///
/// Args:
///     module: The module being built.
///
/// Returns:
///     Nothing.
#[pymodule]
fn _lib(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(evaluate, module)?)?;
    module.add_function(wrap_pyfunction!(evaluate_file, module)?)?;
    Ok(())
}
