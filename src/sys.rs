//! The mruby C API, declared by hand.
//!
//! A dozen functions and two opaque pointers: this is the whole surface the
//! engine needs, so it is written out rather than generated. A generator
//! would bring a build-time dependency and a wall of names nobody reads, and
//! it would still not say which of them we are allowed to call.
//!
//! Everything here is `unsafe` by nature; the safe shell over it is
//! `crate::engine`. The one rule that matters at this level: an `mrb_value`
//! is only valid while its `mrb_state` lives, and only on the thread that
//! made it.

use std::ffi::{c_char, c_int, c_void};

/// The interpreter. Opaque: we only ever hold a pointer to it.
#[repr(C)]
pub struct MrbState {
    _private: [u8; 0],
}

/// A parser/codegen context, which carries the file name for backtraces.
#[repr(C)]
pub struct MrbcContext {
    _private: [u8; 0],
}

/// One Ruby value, boxed the way mruby's default build boxes them.
///
/// mruby has three boxing schemes chosen at build time; the default word
/// boxing on a 64-bit host is a single word, and `build_config.rb` is what
/// keeps that true. The value is opaque here — everything we do with one
/// goes through a C function that knows how to read it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct MrbValue {
    pub word: usize,
}

unsafe extern "C" {
    pub fn mrb_open() -> *mut MrbState;
    pub fn mrb_close(mrb: *mut MrbState);

    pub fn mrb_ccontext_new(mrb: *mut MrbState) -> *mut MrbcContext;
    pub fn mrb_ccontext_free(mrb: *mut MrbState, context: *mut MrbcContext);
    pub fn mrb_ccontext_filename(mrb: *mut MrbState, context: *mut MrbcContext, name: *const c_char) -> *const c_char;

    pub fn mrb_load_string_cxt(mrb: *mut MrbState, source: *const c_char, context: *mut MrbcContext) -> MrbValue;

    pub fn mrb_str_to_cstr(mrb: *mut MrbState, value: MrbValue) -> *const c_char;
    pub fn mrb_obj_as_string(mrb: *mut MrbState, value: MrbValue) -> MrbValue;
    pub fn mrb_inspect(mrb: *mut MrbState, value: MrbValue) -> MrbValue;

    /// Trampolines from `shim.c`: these are macros in mruby's headers, so
    /// there is no symbol to call without one.
    pub fn johnny_mrb_test(value: MrbValue) -> c_int;
    pub fn johnny_mrb_string_p(value: MrbValue) -> c_int;
    pub fn johnny_mrb_nil() -> MrbValue;

    /// The pending exception, or nil. `mrb->exc` is a struct field rather
    /// than a function, so reading it needs a trampoline like the macros do.
    pub fn johnny_mrb_exception(mrb: *mut MrbState) -> MrbValue;
    pub fn johnny_mrb_clear_exception(mrb: *mut MrbState);

    /// Compile source to bytecode, or null with the exception on the state.
    /// Bytecode outlives the interpreter that made it, which is the whole
    /// point: compile a vocabulary once, load it into every evaluation.
    pub fn johnny_mrb_compile(
        mrb: *mut MrbState,
        source: *const c_char,
        name: *const c_char,
        size: *mut usize,
    ) -> *mut u8;
    pub fn johnny_mrb_free(mrb: *mut MrbState, pointer: *mut c_void);

    /// Load bytecode into an interpreter and run it.
    pub fn mrb_load_irep_buf_cxt(
        mrb: *mut MrbState,
        bytes: *const c_void,
        size: usize,
        context: *mut MrbcContext,
    ) -> MrbValue;
}
