/* JSON 生成。
 *
 * 写在 C 里有两个理由：转义要精确（引号、反斜杠、控制字符，而中文按
 * UTF-8 原样过去），以及它是每次都要跑的一步。Ruby 侧拼字符串也能做，
 * 但那是把最该确定的一段交给了最容易写错的写法。
 *
 * 只有生成没有解析 —— 这个方向上要的是「把结构写出去」，读回来是宿主
 * 语言的事。不做的事不假装能做。
 */

#include <mruby.h>
#include <mruby/array.h>
#include <mruby/hash.h>
#include <mruby/string.h>
#include <mruby/value.h>
#include <mruby/variable.h>

/* 结构深了多半是自己引用了自己 —— 那种时候栈会先炸，不如在这里说清楚。 */
#define JSON_MAX_DEPTH 64

static void json_write(mrb_state *mrb, mrb_value out, mrb_value value, int depth);

/* 一个字符串按 JSON 的规矩写出去 */
static void json_quote(mrb_state *mrb, mrb_value out, const char *text, mrb_int length) {
  mrb_str_cat_lit(mrb, out, "\"");
  for (mrb_int i = 0; i < length; i++) {
    unsigned char c = (unsigned char)text[i];
    switch (c) {
      case '"':  mrb_str_cat_lit(mrb, out, "\\\""); break;
      case '\\': mrb_str_cat_lit(mrb, out, "\\\\"); break;
      case '\n': mrb_str_cat_lit(mrb, out, "\\n"); break;
      case '\r': mrb_str_cat_lit(mrb, out, "\\r"); break;
      case '\t': mrb_str_cat_lit(mrb, out, "\\t"); break;
      case '\b': mrb_str_cat_lit(mrb, out, "\\b"); break;
      case '\f': mrb_str_cat_lit(mrb, out, "\\f"); break;
      default:
        if (c < 0x20) {
          char escaped[7];
          snprintf(escaped, sizeof(escaped), "\\u%04x", c);
          mrb_str_cat_cstr(mrb, out, escaped);
        }
        else {
          /* 0x20 及以上原样 —— UTF-8 的中文就这么过去，不转成 \uXXXX */
          mrb_str_cat(mrb, out, (const char *)&c, 1);
        }
    }
  }
  mrb_str_cat_lit(mrb, out, "\"");
}

/* 数字和别的东西：交给它自己的 to_s，Ruby 怎么写我们就怎么写 */
static void json_cat_to_s(mrb_state *mrb, mrb_value out, mrb_value value) {
  mrb_value rendered = mrb_funcall_id(mrb, value, MRB_SYM(to_s), 0);
  mrb_str_cat_str(mrb, out, rendered);
}

static void json_write(mrb_state *mrb, mrb_value out, mrb_value value, int depth) {
  if (depth > JSON_MAX_DEPTH) {
    mrb_raise(mrb, E_ARGUMENT_ERROR, "IR 套得太深了（自己引用了自己？）");
  }

  switch (mrb_type(value)) {
    case MRB_TT_FALSE:
      mrb_str_cat_cstr(mrb, out, mrb_nil_p(value) ? "null" : "false");
      return;
    case MRB_TT_TRUE:
      mrb_str_cat_lit(mrb, out, "true");
      return;
    case MRB_TT_INTEGER:
    case MRB_TT_FLOAT:
      json_cat_to_s(mrb, out, value);
      return;
    case MRB_TT_STRING:
      json_quote(mrb, out, RSTRING_PTR(value), RSTRING_LEN(value));
      return;
    case MRB_TT_SYMBOL: {
      mrb_int length;
      const char *name = mrb_sym_name_len(mrb, mrb_symbol(value), &length);
      json_quote(mrb, out, name, length);
      return;
    }
    case MRB_TT_ARRAY: {
      mrb_str_cat_lit(mrb, out, "[");
      mrb_int length = RARRAY_LEN(value);
      for (mrb_int i = 0; i < length; i++) {
        if (i) mrb_str_cat_lit(mrb, out, ",");
        json_write(mrb, out, mrb_ary_entry(value, i), depth + 1);
      }
      mrb_str_cat_lit(mrb, out, "]");
      return;
    }
    case MRB_TT_HASH: {
      mrb_str_cat_lit(mrb, out, "{");
      mrb_value keys = mrb_hash_keys(mrb, value);
      mrb_int length = RARRAY_LEN(keys);
      for (mrb_int i = 0; i < length; i++) {
        if (i) mrb_str_cat_lit(mrb, out, ",");
        mrb_value key = mrb_ary_entry(keys, i);
        /* 键一律是字符串 —— JSON 没有别的键 */
        mrb_value spelled = mrb_funcall_id(mrb, key, MRB_SYM(to_s), 0);
        json_quote(mrb, out, RSTRING_PTR(spelled), RSTRING_LEN(spelled));
        mrb_str_cat_lit(mrb, out, ":");
        json_write(mrb, out, mrb_hash_get(mrb, value, key), depth + 1);
      }
      mrb_str_cat_lit(mrb, out, "}");
      return;
    }
    default: {
      /* 认不出来的按它的 to_s 当字符串写 —— 比悄悄丢掉强 */
      mrb_value rendered = mrb_funcall_id(mrb, value, MRB_SYM(to_s), 0);
      json_quote(mrb, out, RSTRING_PTR(rendered), RSTRING_LEN(rendered));
      return;
    }
  }
}

/* JSON.generate(value) -> String */
static mrb_value json_generate(mrb_state *mrb, mrb_value self) {
  mrb_value value;
  mrb_get_args(mrb, "o", &value);
  mrb_value out = mrb_str_new_capa(mrb, 256);
  json_write(mrb, out, value, 0);
  return out;
}

void mrb_mruby_json_gem_init(mrb_state *mrb) {
  struct RClass *json = mrb_define_module_id(mrb, MRB_SYM(JSON));
  mrb_define_class_method_id(mrb, json, MRB_SYM(generate), json_generate, MRB_ARGS_REQ(1));
}

void mrb_mruby_json_gem_final(mrb_state *mrb) { (void)mrb; }
