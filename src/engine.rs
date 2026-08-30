//! A safe shell over one mruby interpreter.
//!
//! This is a **virtual machine and nothing else**: run some chunks of Ruby
//! in order, then one more expression, and hand back the string that last
//! expression produced. Whatever those chunks define — words, modules, an
//! IR — belongs to the caller. The engine has no idea and no opinion.
//!
//! A chunk may be source or **bytecode**. Bytecode does not belong to the
//! interpreter that ran it, so a chunk the caller runs again and again is
//! compiled once and loaded after — which is what keeps "the language is
//! defined by whoever uses it" from meaning "parse it on every file".
//!
//! One interpreter per evaluation, opened and closed. A file is evaluated
//! in a millisecond and an interpreter is cheap; sharing one would let one
//! file's constants leak into the next, which is the kind of quiet
//! difference this project refuses everywhere else.

use std::ffi::{c_void, CStr, CString};
use std::sync::{Mutex, MutexGuard};

use crate::sys;

/// Held for the length of a run.
///
/// mruby's Prism-based compiler keeps **global** state: `mrc_init_presym`
/// writes a file-static `offset` every time an interpreter compiles
/// something, so two interpreters compiling at once corrupt each other's
/// symbol ids (a debug build asserts; a release build quietly gets the
/// wrong symbols). Interpreters are otherwise independent, so the lock is
/// only around the compiling — which is the whole of a run.
///
/// A run is a millisecond. Serialising them costs nothing next to being
/// wrong in a way that would surface as a mystery three layers up.
static COMPILING: Mutex<()> = Mutex::new(());

/// Take the compiler lock, ignoring a previous panic's poison.
///
/// A panicking run leaves no shared state behind — the poison would only
/// stop later runs from working for no reason.
fn compiling() -> MutexGuard<'static, ()> {
    COMPILING.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// What went wrong, in words the author of the source can act on.
#[derive(Debug)]
pub struct Failure(pub String);

impl std::fmt::Display for Failure {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(&self.0)
    }
}

/// One chunk to run: Ruby source, or bytecode `compile` produced.
pub enum Chunk<'a> {
    /// Source, parsed now.
    Source(&'a str, &'a str),
    /// Bytecode, compiled somewhere else.
    Bytecode(&'a [u8], &'a str),
}

/// An open interpreter, closed when it drops.
pub struct Interpreter(*mut sys::MrbState);

impl Interpreter {
    /// Open one.
    ///
    /// # Errors
    ///
    /// When mruby cannot allocate a state.
    pub fn open() -> Result<Self, Failure> {
        // SAFETY: `mrb_open` allocates a state or returns null.
        let state = unsafe { sys::mrb_open() };
        if state.is_null() {
            return Err(Failure("mruby would not open (out of memory?)".into()));
        }
        Ok(Self(state))
    }

    /// Open one compile context, shared by every chunk of a run.
    ///
    /// Shared on purpose: a context carries the local variables a chunk
    /// declared, so chunks run in order behave like one program rather than
    /// like unrelated files that happen to share an interpreter. It is what
    /// mruby's own shell does between lines.
    ///
    /// # Errors
    ///
    /// When mruby cannot allocate one.
    pub fn context(&self) -> Result<Context<'_>, Failure> {
        // SAFETY: the state is open; the context is freed when it drops.
        let context = unsafe { sys::mrb_ccontext_new(self.0) };
        if context.is_null() {
            return Err(Failure("mruby would not make a compile context".into()));
        }
        Ok(Context { interpreter: self, raw: context })
    }

    /// Set one global variable to a string, `$name` style.
    ///
    /// How a caller hands a context over: a JSON blob lands in a global
    /// and the Ruby side parses it lazily. Through a global rather than
    /// interpolated into source, because a string in a global has no
    /// escaping problem.
    ///
    /// # Errors
    ///
    /// When the name or value carries a NUL byte.
    pub fn set_global(&self, name: &str, value: &str) -> Result<(), Failure> {
        let named = CString::new(name).map_err(|_| Failure("the name carries a NUL byte".into()))?;
        // SAFETY: the state is open; the bytes are copied by mruby.
        unsafe {
            sys::johnny_mrb_set_global(self.0, named.as_ptr(), value.as_ptr() as *const _, value.len());
        }
        Ok(())
    }

    /// Compile source to bytecode.
    ///
    /// # Errors
    ///
    /// When the source does not compile.
    pub fn compile(&self, source: &str, name: &str) -> Result<Vec<u8>, Failure> {
        let source = CString::new(source).map_err(|_| Failure("the source carries a NUL byte".into()))?;
        let named = CString::new(name).map_err(|_| Failure("the name carries a NUL byte".into()))?;

        // SAFETY: mruby's buffer is copied out and freed here on every path.
        unsafe {
            let mut size: usize = 0;
            let bytes = sys::johnny_mrb_compile(self.0, source.as_ptr(), named.as_ptr(), &mut size);
            if bytes.is_null() {
                let exception = sys::johnny_mrb_exception(self.0);
                let told = if sys::johnny_mrb_test(exception) != 0 {
                    let rendered = self.render(exception);
                    sys::johnny_mrb_clear_exception(self.0);
                    rendered
                } else {
                    "it does not compile".into()
                };
                return Err(Failure(format!("{name}: {told}")));
            }
            let out = std::slice::from_raw_parts(bytes, size).to_vec();
            sys::johnny_mrb_free(self.0, bytes as *mut c_void);
            Ok(out)
        }
    }

    /// Read what an evaluation left, or the exception it raised.
    ///
    /// # Safety
    ///
    /// The value must belong to this interpreter and still be live.
    unsafe fn landed(&self, value: sys::MrbValue, name: &str) -> Result<Option<String>, Failure> {
        unsafe {
            let exception = sys::johnny_mrb_exception(self.0);
            if sys::johnny_mrb_test(exception) != 0 {
                let rendered = self.render(exception);
                sys::johnny_mrb_clear_exception(self.0);
                // Ruby's parse errors name no file, so the file goes in
                // front: an engine that cannot say *which* file failed is no
                // better than the YAML it replaced.
                return Err(Failure(format!("{name}: {rendered}")));
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
            self.text(rendered).unwrap_or_else(|| "(an error that will not render)".into())
        }
    }
}

/// One compile context, and the chunks run through it.
pub struct Context<'a> {
    interpreter: &'a Interpreter,
    raw: *mut sys::MrbcContext,
}

impl Context<'_> {
    /// Evaluate one chunk of source, named for backtraces.
    ///
    /// # Errors
    ///
    /// When the source does not parse, or raises.
    pub fn eval(&self, source: &str, name: &str) -> Result<Option<String>, Failure> {
        let source = CString::new(source).map_err(|_| Failure("the source carries a NUL byte".into()))?;
        let named = CString::new(name).map_err(|_| Failure("the name carries a NUL byte".into()))?;

        // SAFETY: the state and context are both live; the exception is read
        // before any value is touched.
        unsafe {
            let state = self.interpreter.0;
            sys::mrb_ccontext_filename(state, self.raw, named.as_ptr());
            let value = sys::mrb_load_string_cxt(state, source.as_ptr(), self.raw);
            self.interpreter.landed(value, name)
        }
    }

    /// Load and run bytecode a previous compilation produced.
    ///
    /// # Errors
    ///
    /// When the bytecode does not load, or raises.
    pub fn load(&self, bytecode: &[u8], name: &str) -> Result<Option<String>, Failure> {
        let named = CString::new(name).map_err(|_| Failure("the name carries a NUL byte".into()))?;

        // SAFETY: as above; the buffer is read, never kept.
        unsafe {
            let state = self.interpreter.0;
            sys::mrb_ccontext_filename(state, self.raw, named.as_ptr());
            let value = sys::mrb_load_irep_buf_cxt(
                state,
                bytecode.as_ptr() as *const c_void,
                bytecode.len(),
                self.raw,
            );
            self.interpreter.landed(value, name)
        }
    }
}

impl Drop for Context<'_> {
    fn drop(&mut self) {
        // SAFETY: made by `context`, freed once, while the state still lives.
        unsafe { sys::mrb_ccontext_free(self.interpreter.0, self.raw) }
    }
}

impl Drop for Interpreter {
    fn drop(&mut self) {
        // SAFETY: the state was opened here and is closed once.
        unsafe { sys::mrb_close(self.0) }
    }
}

/// Run chunks in order, then one expression, and return its string.
///
/// Args:
///   chunks: What to run, in order. Each is source or bytecode, with the
///     name it should carry in a backtrace.
///   answer: The expression run last, whose string value comes back.
///   given: A context handed to the program as the global `$johnny_context`
///     before anything runs — a JSON blob by convention, though the engine
///     neither parses nor cares.
///
/// Returns:
///   Whatever the answer expression produced.
///
/// Errors:
///   A `Failure` naming the chunk and carrying Ruby's own message.
pub fn run(chunks: &[Chunk<'_>], answer: &str, given: Option<&str>) -> Result<String, Failure> {
    let _serialised = compiling();
    let interpreter = Interpreter::open()?;
    if let Some(value) = given {
        interpreter.set_global("$johnny_context", value)?;
    }
    let context = interpreter.context()?;
    for chunk in chunks {
        match chunk {
            Chunk::Source(text, name) => context.eval(text, name)?,
            Chunk::Bytecode(bytes, name) => context.load(bytes, name)?,
        };
    }
    match context.eval(answer, "<answer>")? {
        Some(text) => Ok(text),
        None => Err(Failure(format!("{answer} produced no string"))),
    }
}

/// Compile source to bytecode, for a chunk that will be run often.
///
/// Args:
///   source: What to compile.
///   name: What to call it in a backtrace.
///
/// Returns:
///   The bytecode.
///
/// Errors:
///   A `Failure` when it does not compile.
pub fn compile(source: &str, name: &str) -> Result<Vec<u8>, Failure> {
    let _serialised = compiling();
    Interpreter::open()?.compile(source, name)
}
