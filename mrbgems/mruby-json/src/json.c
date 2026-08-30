/* JSON, both directions.
 *
 * Written in C for two reasons: the escaping has to be exact (quotes,
 * backslashes, control characters - while UTF-8 text passes through
 * untouched), and it runs on every call. Ruby-side string building could
 * do it, but that hands the part that most needs to be right to the way
 * of writing it that is easiest to get wrong.
 *
 * Generation: JSON.generate / #to_json. Parsing: JSON.parse - object keys
 * become Strings, numbers become Integer when they fit and Float
 * otherwise, errors are ArgumentError naming the byte offset.
 */

#include <stdlib.h>
#include <string.h>

#include <mruby.h>
#include <mruby/array.h>
#include <mruby/hash.h>
#include <mruby/string.h>
#include <mruby/value.h>
#include <mruby/variable.h>

/* see header comment */
#define JSON_MAX_DEPTH 64

static void json_write(mrb_state *mrb, mrb_value out, mrb_value value, int depth);

/* see header comment */
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
          /* see header comment */
          mrb_str_cat(mrb, out, (const char *)&c, 1);
        }
    }
  }
  mrb_str_cat_lit(mrb, out, "\"");
}

/* see header comment */
static void json_cat_to_s(mrb_state *mrb, mrb_value out, mrb_value value) {
  mrb_value rendered = mrb_funcall_id(mrb, value, MRB_SYM(to_s), 0);
  mrb_str_cat_str(mrb, out, rendered);
}

static void json_write(mrb_state *mrb, mrb_value out, mrb_value value, int depth) {
  if (depth > JSON_MAX_DEPTH) {
    mrb_raise(mrb, E_ARGUMENT_ERROR, "nested too deeply (a structure containing itself?)");
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
        /* see header comment */
        mrb_value spelled = mrb_funcall_id(mrb, key, MRB_SYM(to_s), 0);
        json_quote(mrb, out, RSTRING_PTR(spelled), RSTRING_LEN(spelled));
        mrb_str_cat_lit(mrb, out, ":");
        json_write(mrb, out, mrb_hash_get(mrb, value, key), depth + 1);
      }
      mrb_str_cat_lit(mrb, out, "}");
      return;
    }
    default: {
      /* An unknown type is written as its to_s, quoted - better than dropped. */
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


/* ---- Parsing ----------------------------------------------------------
 *
 * The other half of the gem: JSON text in, Ruby values out. Object keys
 * become Strings, numbers become Integer when they fit and Float
 * otherwise, \uXXXX escapes decode to UTF-8 (surrogate pairs included).
 * Errors are ArgumentError naming the byte offset - a context blob that
 * does not parse must say where, not just that.
 */

typedef struct {
  mrb_state *mrb;
  const char *p;
  const char *end;
  const char *begin;
  int depth;
} json_reader;

static mrb_value json_read_value(json_reader *j);

static void json_read_fail(json_reader *j, const char *what) {
  mrb_state *mrb = j->mrb; /* E_ARGUMENT_ERROR expands to a use of `mrb` */
  mrb_raisef(mrb, E_ARGUMENT_ERROR, "JSON.parse: %s at byte %d", what, (int)(j->p - j->begin));
}

static void json_read_ws(json_reader *j) {
  while (j->p < j->end && (*j->p == ' ' || *j->p == '\t' || *j->p == '\n' || *j->p == '\r')) j->p++;
}

static int json_read_hex(json_reader *j) {
  if (j->p >= j->end) json_read_fail(j, "truncated \\u escape");
  char c = *j->p++;
  if (c >= '0' && c <= '9') return c - '0';
  if (c >= 'a' && c <= 'f') return c - 'a' + 10;
  if (c >= 'A' && c <= 'F') return c - 'A' + 10;
  json_read_fail(j, "bad \\u escape");
  return 0;
}

static void json_read_codepoint(json_reader *j, mrb_value out, unsigned long cp) {
  char b[4];
  int n;
  if (cp < 0x80) { b[0] = (char)cp; n = 1; }
  else if (cp < 0x800) { b[0] = (char)(0xC0 | (cp >> 6)); b[1] = (char)(0x80 | (cp & 0x3F)); n = 2; }
  else if (cp < 0x10000) {
    b[0] = (char)(0xE0 | (cp >> 12));
    b[1] = (char)(0x80 | ((cp >> 6) & 0x3F));
    b[2] = (char)(0x80 | (cp & 0x3F));
    n = 3;
  } else {
    b[0] = (char)(0xF0 | (cp >> 18));
    b[1] = (char)(0x80 | ((cp >> 12) & 0x3F));
    b[2] = (char)(0x80 | ((cp >> 6) & 0x3F));
    b[3] = (char)(0x80 | (cp & 0x3F));
    n = 4;
  }
  mrb_str_cat(j->mrb, out, b, n);
}

static mrb_value json_read_string(json_reader *j) {
  j->p++; /* opening quote */
  mrb_value out = mrb_str_new(j->mrb, NULL, 0);
  const char *chunk = j->p;
  for (;;) {
    if (j->p >= j->end) json_read_fail(j, "unterminated string");
    unsigned char c = (unsigned char)*j->p;
    if (c == '"') {
      mrb_str_cat(j->mrb, out, chunk, j->p - chunk);
      j->p++;
      return out;
    }
    if (c == '\\') {
      mrb_str_cat(j->mrb, out, chunk, j->p - chunk);
      j->p++;
      if (j->p >= j->end) json_read_fail(j, "truncated escape");
      char e = *j->p++;
      switch (e) {
        case '"': mrb_str_cat(j->mrb, out, "\"", 1); break;
        case '\\': mrb_str_cat(j->mrb, out, "\\\\", 1); break;
        case '/': mrb_str_cat(j->mrb, out, "/", 1); break;
        case 'b': mrb_str_cat(j->mrb, out, "\b", 1); break;
        case 'f': mrb_str_cat(j->mrb, out, "\f", 1); break;
        case 'n': mrb_str_cat(j->mrb, out, "\n", 1); break;
        case 'r': mrb_str_cat(j->mrb, out, "\r", 1); break;
        case 't': mrb_str_cat(j->mrb, out, "\t", 1); break;
        case 'u': {
          unsigned long cp = 0;
          int i;
          for (i = 0; i < 4; i++) cp = (cp << 4) | (unsigned long)json_read_hex(j);
          if (cp >= 0xD800 && cp <= 0xDBFF) {
            if (j->end - j->p >= 2 && j->p[0] == '\\' && j->p[1] == 'u') {
              unsigned long lo = 0;
              j->p += 2;
              for (i = 0; i < 4; i++) lo = (lo << 4) | (unsigned long)json_read_hex(j);
              if (lo < 0xDC00 || lo > 0xDFFF) json_read_fail(j, "bad surrogate pair");
              cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
            } else {
              json_read_fail(j, "lone high surrogate");
            }
          } else if (cp >= 0xDC00 && cp <= 0xDFFF) {
            json_read_fail(j, "lone low surrogate");
          }
          json_read_codepoint(j, out, cp);
          break;
        }
        default: json_read_fail(j, "bad escape");
      }
      chunk = j->p;
    } else if (c < 0x20) {
      json_read_fail(j, "control character in string");
    } else {
      j->p++; /* UTF-8 bytes pass through untouched */
    }
  }
}

static mrb_value json_read_number(json_reader *j) {
  char buf[64];
  const char *start = j->p;
  int is_int = 1;
  if (j->p < j->end && *j->p == '-') j->p++;
  while (j->p < j->end && *j->p >= '0' && *j->p <= '9') j->p++;
  if (j->p < j->end && *j->p == '.') {
    is_int = 0;
    j->p++;
    while (j->p < j->end && *j->p >= '0' && *j->p <= '9') j->p++;
  }
  if (j->p < j->end && (*j->p == 'e' || *j->p == 'E')) {
    is_int = 0;
    j->p++;
    if (j->p < j->end && (*j->p == '+' || *j->p == '-')) j->p++;
    while (j->p < j->end && *j->p >= '0' && *j->p <= '9') j->p++;
  }
  size_t length = (size_t)(j->p - start);
  if (length == 0 || length >= sizeof(buf)) json_read_fail(j, "bad number");
  memcpy(buf, start, length);
  buf[length] = 0;
  if (is_int) {
    char *tail = NULL;
    long long v = strtoll(buf, &tail, 10);
    if (tail == buf + length && v >= (long long)MRB_INT_MIN && v <= (long long)MRB_INT_MAX) {
      return mrb_int_value(j->mrb, (mrb_int)v);
    }
  }
  return mrb_float_value(j->mrb, strtod(buf, NULL));
}

static mrb_value json_read_literal(json_reader *j, const char *word, size_t length, mrb_value value) {
  if ((size_t)(j->end - j->p) < length || memcmp(j->p, word, length) != 0) {
    json_read_fail(j, "bad literal");
  }
  j->p += length;
  return value;
}

static mrb_value json_read_value(json_reader *j) {
  if (j->depth > JSON_MAX_DEPTH) json_read_fail(j, "nested too deeply");
  json_read_ws(j);
  if (j->p >= j->end) json_read_fail(j, "unexpected end");
  switch (*j->p) {
    case '"': return json_read_string(j);
    case '{': {
      mrb_value out = mrb_hash_new(j->mrb);
      j->p++;
      j->depth++;
      json_read_ws(j);
      if (j->p < j->end && *j->p == '}') { j->p++; j->depth--; return out; }
      for (;;) {
        int arena = mrb_gc_arena_save(j->mrb);
        json_read_ws(j);
        if (j->p >= j->end || *j->p != '"') json_read_fail(j, "expected a key");
        mrb_value key = json_read_string(j);
        json_read_ws(j);
        if (j->p >= j->end || *j->p != ':') json_read_fail(j, "expected ':'");
        j->p++;
        mrb_value value = json_read_value(j);
        mrb_hash_set(j->mrb, out, key, value);
        mrb_gc_arena_restore(j->mrb, arena);
        json_read_ws(j);
        if (j->p < j->end && *j->p == ',') { j->p++; continue; }
        if (j->p < j->end && *j->p == '}') { j->p++; j->depth--; return out; }
        json_read_fail(j, "expected ',' or '}'");
      }
    }
    case '[': {
      mrb_value out = mrb_ary_new(j->mrb);
      j->p++;
      j->depth++;
      json_read_ws(j);
      if (j->p < j->end && *j->p == ']') { j->p++; j->depth--; return out; }
      for (;;) {
        int arena = mrb_gc_arena_save(j->mrb);
        mrb_value value = json_read_value(j);
        mrb_ary_push(j->mrb, out, value);
        mrb_gc_arena_restore(j->mrb, arena);
        json_read_ws(j);
        if (j->p < j->end && *j->p == ',') { j->p++; continue; }
        if (j->p < j->end && *j->p == ']') { j->p++; j->depth--; return out; }
        json_read_fail(j, "expected ',' or ']'");
      }
    }
    case 't': return json_read_literal(j, "true", 4, mrb_true_value());
    case 'f': return json_read_literal(j, "false", 5, mrb_false_value());
    case 'n': return json_read_literal(j, "null", 4, mrb_nil_value());
    default: return json_read_number(j);
  }
}

/* JSON.parse(text) -> value */
static mrb_value json_parse(mrb_state *mrb, mrb_value self) {
  const char *text;
  mrb_int length;
  mrb_get_args(mrb, "s", &text, &length);
  json_reader j = { mrb, text, text + length, text, 0 };
  mrb_value out = json_read_value(&j);
  json_read_ws(&j);
  if (j.p != j.end) json_read_fail(&j, "trailing garbage");
  return out;
}

void mrb_mruby_json_gem_init(mrb_state *mrb) {
  struct RClass *json = mrb_define_module_id(mrb, MRB_SYM(JSON));
  mrb_define_class_method_id(mrb, json, MRB_SYM(generate), json_generate, MRB_ARGS_REQ(1));
  mrb_define_class_method_id(mrb, json, MRB_SYM(parse), json_parse, MRB_ARGS_REQ(1));
}

void mrb_mruby_json_gem_final(mrb_state *mrb) { (void)mrb; }
