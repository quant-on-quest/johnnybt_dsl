# mruby-johnnybt

策略 DSL 的词汇表：`strategy` / `template` / `selector` / `count` /
`period` / `params` / `factors` / `filters` / `meta` / `window`。

写在 `mrblib/` 里，构建时编成字节码进 `libmruby.a` —— 每次求值不必再解析
一遍，也就不存在「词汇表在运行时语法错」这种事。序列化归 `mruby-json`。

DSL 词汇之外的东西一律当模板参数绑上去（`method_missing`）：模板的签名
才是那份权威清单，在这里再抄一遍只会漂移，拼错的键由 Python 那侧按签名报错。
