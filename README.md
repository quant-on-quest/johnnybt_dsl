# johnny_dsl

一台装好的 **mruby 虚拟机**。跑几段 Ruby，取回它产出的那个字符串 —— 就这些。

```python
from johnny_dsl import compile, run

words = compile(open("dsl.rb").read(), "dsl.rb")     # 一个进程编一次
ir = run([(words, "dsl.rb"), (source, "策略.rb")], answer="MyDSL.result")
```

**词是你自己的。** 这个包不知道什么是策略、什么是因子，也不规定 IR 长什么样：
那都是拿它写 DSL 的项目说了算。写死一套词就成了只服务一个项目的库。

**装上就能用**：wheel 里 mruby 已经编好了，用的人**不需要 GCC，也不需要 Ruby**。
那两样只有我们构建 wheel 时才要。

## API

| | |
|---|---|
| `compile(source, name) -> bytes` | 编成字节码。字节码和解释器无关，编一次到处 load |
| `run(chunks, answer) -> str` | 按顺序跑几段（源码或字节码），再跑 `answer`，返回它的字符串 |
| `run_file(path, before, answer)` | 同上，最后一段读自文件 |

一次 `run` 一个解释器：一份文件定义的东西不会漏进下一份。几段之间共用一个
编译上下文，所以局部变量跨得过去 —— 它们像一段程序，不像几份互不相干的文件。

## 里面有什么

| 件 | 做什么 |
|---|---|
| `build.rs` | 取 mruby（钉住某个 commit）、按 `build_config.rb` 编、链接 `libmruby.a` |
| `src/sys.rs` | mruby 的 C API，**手写**的 extern 声明。十来个函数，不用生成器 |
| `src/shim.c` | mruby 里是宏的那几个（`mrb_test` / `mrb->exc`）和编译入口的跳板 |
| `src/engine.rs` | 解释器的生命周期，以及「跑几段、取一个字符串」 |
| `mrbgems/mruby-json/` | 我们自己的 gem：`JSON.generate` / `#to_json`，生成在 C 里 |
| `mrbgems/mruby-dsl/` | 我们自己的 gem：`JohnnyDSL::Base` —— DSL 的基类（注册、declare、逐语句 trouble、manifest/ir 信封）。**词不在这里**，词归定义语言的项目 |

## 一处 mruby 的实情

mruby 的 Prism 编译器带**全局状态**（`mrc_init_presym` 写一个文件级的
`offset`），两个解释器同时编译会互相踩。所以 `run` / `compile` 用一把锁串
起来 —— 一次求值不到一毫秒，串行的代价远小于「错得莫名其妙」。

## 构建（只有我们要）

* Rust、C 编译器
* Ruby —— mruby 自己的 rake 要它（`brew install ruby`）。解析器是 Prism，**不要 bison**

构建走真 `rake -m`（并行）：mruby 自带的 `minirake` 完全串行，316 个目标文件
一个一个编，32 核机器上要几分钟；`rake -m` 同一棵树 **4 秒**。没有 rake 时退回
minirake。`vendor/mruby` 不进 git，第一次 `cargo build` 自己取。

测试：`vendor/mruby/build/host/bin/mruby mrbgems/run_tests.rb`（gem 那侧）与
`cargo test`（引擎那侧）。
