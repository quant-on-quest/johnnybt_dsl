# mruby-json

JSON 生成：`JSON.generate(x)` / `JSON.dump(x)` / `x.to_json`。

只有生成，没有解析 —— 这个方向上要的是「把结构写出去」，读回来是宿主
语言的事（Python 有它自己的 json）。不做的事不假装能做。

生成在 C 里（`src/json.c`）：转义要精确（引号、反斜杠、控制字符），
中文按 UTF-8 原样过去不转 `\uXXXX`，自己引用自己的结构在 64 层处被拒绝
而不是把栈跑穿。
