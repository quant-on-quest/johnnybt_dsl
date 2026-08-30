# johnny_dsl

策略 DSL 的引擎：**mruby 跑在 Rust 里，回到 Python 的只有一段 JSON IR**。

一份策略是一段程序，所以写它的该是一门语言。YAML 表达不了循环和条件，
用 Python 硬凑 DSL 又要处处迁就 Python 的语法；把这件事交给 Ruby，两边
就都不必将就 —— DSL 怎么写是 Ruby 的事，IR 怎么解释是 Python 的事。

```ruby
strategy "小市值·周黎明" do
  template :官方选股
  selector :周黎明

  count 10
  period "W", offsets: [0]

  params do
    短动量入围 100
    强势行业数 2
    长动量分位下限 0.2
  end

  factors do
    use "price_volume.动量", n: 5, as: :短动量, ascending: false
    use "base.市值", as: :市值, ascending: true
  end

  filters do
    keep "规模.成交额均值", n: 5, at_least: 5000_0000
  end
end
```

它是真的 Ruby：一族策略写成 `each`，共用的一段写成方法，阈值写成常量。

```ruby
[5, 10, 20].each do |n|
  strategy "动量#{n}" do
    template :官方选股
    factors { use "price_volume.动量", n: n, ascending: false }
  end
end
```

## 构成

| 件 | 做什么 |
|---|---|
| `build.rs` | 取 mruby 源码（钉住某个 commit）、按 `build_config.rb` 编译、链接 `libmruby.a` |
| `src/sys.rs` | mruby 的 C API，**手写**的 extern 声明。十来个函数，不用生成器 |
| `src/shim.c` | mruby 里是宏的那几个（`mrb_test` / `mrb->exc`）的跳板，按它的头文件编译 |
| `src/engine.rs` | 一次求值 = 开一个解释器、跑 prelude、跑文件、取 IR、关掉 |
| `mrbgems/mruby-json/` | 我们自己的 gem：`JSON.generate` / `#to_json`，生成在 C 里 |
| `mrbgems/mruby-johnnybt/` | 我们自己的 gem：**DSL 的词汇表**，构建时编成字节码 |
| `python/johnny_dsl/` | Python 这侧：`evaluate_file(path) -> dict` |

## 自己的两个 mrbgem

DSL 只活在 mruby 里，所以它就该是 mruby 的扩展，而不是每次求值现解析的
一段字符串：

* **mruby-json** —— 标准写法的 JSON 生成（`JSON.generate` / `#to_json`），
  写在 C 里（转义要精确、每次都要跑）。只有生成没有解析。
* **mruby-johnnybt** —— DSL 的词汇表，写在 `mrblib/` 里，**构建时编成字节码
  进 libmruby.a**：每次求值只解析策略文件本身，词汇表不可能在运行时语法错。

两个 gem 各带自己的 Ruby 测试。跑它们：

```bash
vendor/mruby/build/host/bin/mruby mrbgems/run_tests.rb   # 3 毫秒
```

不走 `rake test` —— 那会把 mruby core 加 full-core 每个 gem 的测试全编进
mrbtest，为二十条断言付整套 mruby 测试的编译费（两分钟对三毫秒）。

## 构建要什么

* Rust（cargo）
* Ruby —— **只在构建时**要，mruby 自己的 rake 需要它（`brew install ruby`）
* C 编译器。解析器是 Prism，所以**不要 bison**

构建用真 `rake -m`（并行）而不是 mruby 自带的 `minirake`：后者完全串行，
316 个目标文件一个一个编，在 32 核的机器上要几分钟；`rake -m` 同一棵树
**4 秒**。没有 rake 时自动退回 minirake。

`vendor/mruby` 不进 git：`cargo build` 第一次会按钉住的 commit 取回来，
也可以 `MRUBY_DIR=/path/to/mruby` 指向本地的树。

## 边界

DSL 拿得到的是完整的语言（Struct、Enumerable、异常、字符串格式化），
拿不到的是进程之外的世界：没有文件 IO、没有 `system`、没有网络。
一份策略描述它自己，不去做事 —— 和因子层不许写 `shift(-1)` 是同一条规矩。
