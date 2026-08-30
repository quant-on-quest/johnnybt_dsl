/* Trampolines for the parts of mruby's API that are macros.
 *
 * `mrb_test`, `mrb_string_p` and friends are inline in mruby's headers, so
 * there is no symbol for Rust to call. Rather than reimplement the boxing
 * rules on the Rust side — where they would silently rot the day mruby's
 * build config changes — each one gets a real function here, compiled
 * against mruby's own headers so it stays whatever mruby says it is.
 */

#include <mruby.h>
#include <mruby/compile.h>
#include <mruby/dump.h>
#include <mruby/internal.h>
#include <mruby/irep.h>
#include <mruby/proc.h>
#include <mruby/string.h>
#include <mruby/variable.h>

int johnny_mrb_test(mrb_value value) { return mrb_test(value) ? 1 : 0; }

int johnny_mrb_string_p(mrb_value value) { return mrb_string_p(value) ? 1 : 0; }

mrb_value johnny_mrb_nil(void) { return mrb_nil_value(); }

/* `mrb->exc` is a field on the state, not a function. */
mrb_value johnny_mrb_exception(mrb_state *mrb) {
  return mrb->exc ? mrb_obj_value(mrb->exc) : mrb_nil_value();
}

void johnny_mrb_clear_exception(mrb_state *mrb) { mrb->exc = NULL; }

/* Compile source to bytecode.
 *
 * Compile once, load anywhere: bytecode does not belong to the interpreter
 * that made it, so a vocabulary is compiled once and loaded into every
 * later evaluation — which is exactly the parse that would otherwise be
 * paid per file.
 *
 * Returns NULL on failure, leaving the exception on the state for the
 * caller to read.
 */
uint8_t *johnny_mrb_compile(mrb_state *mrb, const char *source, const char *name, size_t *size) {
  mrb_ccontext *context = mrb_ccontext_new(mrb);
  if (!context) return NULL;
  mrb_ccontext_filename(mrb, context, name);
  context->no_exec = TRUE;

  struct mrb_parser_state *parser = mrb_parse_string(mrb, source, context);
  if (!parser || parser->nerr > 0) {
    if (parser) mrb_parser_free(parser);
    mrb_ccontext_free(mrb, context);
    return NULL;
  }

  struct RProc *proc = mrb_generate_code(mrb, parser);
  mrb_parser_free(parser);
  if (!proc) {
    mrb_ccontext_free(mrb, context);
    return NULL;
  }

  uint8_t *bytes = NULL;
  int failed = mrb_dump_irep(mrb, proc->body.irep, MRB_DUMP_DEBUG_INFO, &bytes, size);
  mrb_ccontext_free(mrb, context);
  return failed ? NULL : bytes;
}

void johnny_mrb_free(mrb_state *mrb, void *pointer) { mrb_free(mrb, pointer); }

/* Set one global variable to a string value.
 *
 * How a context crosses into the interpreter: the host hands a JSON blob,
 * this puts it in a global, and Ruby parses it lazily. A string through a
 * global has no source-escaping problem; interpolating it into generated
 * source would.
 */
void johnny_mrb_set_global(mrb_state *mrb, const char *name, const char *value, size_t size) {
  mrb_gv_set(mrb, mrb_intern_cstr(mrb, name), mrb_str_new(mrb, value, size));
}
