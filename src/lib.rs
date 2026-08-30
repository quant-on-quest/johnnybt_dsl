//! `johnny_dsl` — an mruby virtual machine, ready to use.
//!
//! The package provides the **machine and nothing else**: run some chunks
//! of Ruby, then one expression, and hand back the string it produced.
//! Whatever those chunks define — words, modules, an IR — belongs to the
//! caller. A language baked in here would make one library serve one
//! project.
//!
//! A chunk can be **precompiled** to bytecode, which does not belong to the
//! interpreter that ran it: the caller compiles once and runs it into every
//! evaluation after.
//!
//! What ships is a wheel with mruby already compiled into it — installing
//! it needs neither a C compiler nor Ruby.

mod engine;
mod sys;

pub use engine::{compile as compile_source, run as run_chunks, Chunk, Failure};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

/// Run chunks of Ruby in order, then an expression, and return its string.
///
/// Args:
///     chunks: What to run, in order. Each is `(source_or_bytecode, name)`,
///         or a bare string or bytes named `<chunk>`.
///     answer: The expression run last, whose string value comes back.
///
/// Returns:
///     Whatever the answer produced, as text.
///
/// Raises:
///     ValueError: If a chunk does not run — the message names it and
///         carries Ruby's own error.
#[pyfunction]
#[pyo3(signature = (chunks, answer = "nil.to_s"))]
fn run(chunks: Vec<Bound<'_, PyAny>>, answer: &str) -> PyResult<String> {
    // The owned halves live here so the borrowed `Chunk`s stay valid for
    // the whole call.
    let mut owned: Vec<(Option<String>, Option<Vec<u8>>, String)> = Vec::with_capacity(chunks.len());
    for chunk in &chunks {
        let (body, name) = match chunk.extract::<(Bound<'_, PyAny>, String)>() {
            Ok((body, name)) => (body, name),
            Err(_) => (chunk.clone(), "<chunk>".to_string()),
        };
        // Bytes are bytecode, text is source — the caller need not say which.
        if let Ok(bytes) = body.extract::<Vec<u8>>() {
            owned.push((None, Some(bytes), name));
        } else {
            owned.push((Some(body.extract::<String>()?), None, name));
        }
    }

    let ordered: Vec<engine::Chunk<'_>> = owned
        .iter()
        .map(|(text, bytes, name)| match (text, bytes) {
            (Some(text), _) => engine::Chunk::Source(text.as_str(), name.as_str()),
            (_, Some(bytes)) => engine::Chunk::Bytecode(bytes.as_slice(), name.as_str()),
            _ => unreachable!("a chunk is either source or bytecode"),
        })
        .collect();

    engine::run(&ordered, answer).map_err(|failure| PyValueError::new_err(failure.0))
}

/// Compile Ruby source to bytecode.
///
/// Args:
///     source: What to compile.
///     name: What to call it in a backtrace.
///
/// Returns:
///     The bytecode.
///
/// Raises:
///     ValueError: If it does not compile.
#[pyfunction]
#[pyo3(signature = (source, name = "<chunk>"))]
fn compile<'py>(python: Python<'py>, source: &str, name: &str) -> PyResult<Bound<'py, PyBytes>> {
    let bytes = engine::compile(source, name).map_err(|failure| PyValueError::new_err(failure.0))?;
    Ok(PyBytes::new(python, &bytes))
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
    module.add_function(wrap_pyfunction!(run, module)?)?;
    module.add_function(wrap_pyfunction!(compile, module)?)?;
    Ok(())
}
