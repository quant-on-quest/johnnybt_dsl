/* Trampolines for the parts of mruby's API that are macros.
 *
 * `mrb_test`, `mrb_string_p` and friends are inline in mruby's headers, so
 * there is no symbol for Rust to call. Rather than reimplement the boxing
 * rules on the Rust side — where they would silently rot the day mruby's
 * build config changes — each one gets a real function here, compiled
 * against mruby's own headers so it stays whatever mruby says it is.
 */

#include <mruby.h>
#include <mruby/string.h>

int johnny_mrb_test(mrb_value value) { return mrb_test(value) ? 1 : 0; }

int johnny_mrb_string_p(mrb_value value) { return mrb_string_p(value) ? 1 : 0; }

mrb_value johnny_mrb_nil(void) { return mrb_nil_value(); }

/* `mrb->exc` is a field on the state, not a function. */
mrb_value johnny_mrb_exception(mrb_state *mrb) {
  return mrb->exc ? mrb_obj_value(mrb->exc) : mrb_nil_value();
}

void johnny_mrb_clear_exception(mrb_state *mrb) { mrb->exc = NULL; }
