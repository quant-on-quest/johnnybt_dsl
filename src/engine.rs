//! A safe shell over one mruby interpreter.
//!
//! The whole contract is one function: source in, JSON out. Everything that
//! makes the DSL a DSL — the vocabulary, the error messages, the shape of
//! the IR — is written in Ruby (`ruby/prelude.rb`), because that is the
//! language that reads well for it. This file only holds the interpreter's
//! lifetime and moves two strings across the boundary.
//!
//! One interpreter per evaluation, opened and closed. A strategy file is
//! read in milliseconds and an interpreter is cheap; sharing one would mean
//! one file's constants leaking into the next one's, which is exactly the
//! kind of quiet difference this project refuses elsewhere.

use std::ffi::{CStr, CString};

use crate::sys;

/// The DSL's vocabulary, evaluated before the strategy file itself.
const PRELUDE: &str = include_str!("../ruby/prelude.rb");

/// What went wrong, in words the author of the strategy can act on.
#[derive(Debug)]
pub struct Failure(pub String);

impl std::fmt::Display for Failure {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(&self.0)
    }
}

/// An open interpreter, closed when it drops.
struct Interpreter(*mut sys::MrbState);

impl Interpreter {
    /// Open one.
    fn open() -> Result<Self, Failure> {
        // SAFETY: `mrb_open` allocates a state or returns null.
        let state = unsafe { sys::mrb_open() };
        if state.is_null() {
            return Err(Failure("mruby 开不起来（内存不够？）".into()));
        }
        Ok(Self(state))
    }

    /// Evaluate one chunk of source, named for backtraces.
    ///
    /// Returns whatever the chunk's last expression was, as a string when it
    /// is one.
    fn eval(&self, source: &str, name: &str) -> Result<Option<String>, Failure> {
        let source = CString::new(source).map_err(|_| Failure("源码里有 NUL 字节".into()))?;
        let name = CString::new(name).map_err(|_| Failure("文件名里有 NUL 字节".into()))?;

        // SAFETY: the state is open, and the context is freed on every path
        // out. `mrb_load_string_cxt` leaves any exception on the state, which
        // is read before the values are touched.
        unsafe {
            let context = sys::mrb_ccontext_new(self.0);
            if context.is_null() {
                return Err(Failure("mruby 的编译上下文建不起来".into()));
            }
            sys::mrb_ccontext_filename(self.0, context, name.as_ptr());
            let value = sys::mrb_load_string_cxt(self.0, source.as_ptr(), context);
            sys::mrb_ccontext_free(self.0, context);

            let exception = sys::johnny_mrb_exception(self.0);
            if sys::johnny_mrb_test(exception) != 0 {
                let rendered = self.render(exception);
                sys::johnny_mrb_clear_exception(self.0);
                return Err(Failure(rendered));
            }
            Ok(self.text(value))
        }
    }

    /// Return a value as a Rust string, when it is a string.
    ///
    /// # Safety
    ///
    /// The value must belong to this interpreter and still be live.
    unsafe fn text(&self, value: sys::MrbValue) -> Option<String> {
        unsafe {
            if sys::johnny_mrb_string_p(value) == 0 {
                return None;
            }
            let pointer = sys::mrb_str_to_cstr(self.0, value);
            if pointer.is_null() {
                return None;
            }
            Some(CStr::from_ptr(pointer).to_string_lossy().into_owned())
        }
    }

    /// Render a value the way `to_s` would, for an error message.
    ///
    /// # Safety
    ///
    /// The value must belong to this interpreter and still be live.
    unsafe fn render(&self, value: sys::MrbValue) -> String {
        unsafe {
            let rendered = sys::mrb_obj_as_string(self.0, value);
            self.text(rendered).unwrap_or_else(|| "（说不清的错误）".into())
        }
    }
}

impl Drop for Interpreter {
    fn drop(&mut self) {
        // SAFETY: the state was opened here and is closed once.
        unsafe { sys::mrb_close(self.0) }
    }
}

/// Evaluate a strategy file and return its IR as JSON.
///
/// Args:
///   source: The strategy's own source.
///   name: What to call it in a backtrace — its path, usually.
///
/// Returns:
///   The IR, as JSON text.
///
/// Errors:
///   A `Failure` carrying the Ruby error and its backtrace, or the loader's
///   own complaint when the file declared nothing.
pub fn evaluate(source: &str, name: &str) -> Result<String, Failure> {
    let interpreter = Interpreter::open()?;
    interpreter.eval(PRELUDE, "<johnny_dsl prelude>")?;
    // The prelude wraps the file: Ruby-side rescue turns a strategy author's
    // mistake into an IR carrying the message, so the common error path
    // needs no C-level exception handling at all.
    // Ruby's parse errors name no file, so the file is put in front: an
    // engine that cannot say *which* strategy failed is no better than YAML.
    interpreter
        .eval(source, name)
        .map_err(|failure| Failure(format!("{name}: {}", failure.0)))?;
    match interpreter.eval("JohnnyDSL.__ir__", "<johnny_dsl finish>")? {
        Some(json) => Ok(json),
        None => Err(Failure("DSL 没有产出 IR —— prelude 坏了？".into())),
    }
}
