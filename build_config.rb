# What kind of Ruby the DSL gets.
#
# The full core: a strategy file is a program, so the language behind it is
# the whole one — Struct, Enumerable, Comparable, String formatting, Math,
# Time, exceptions with backtraces. What is deliberately absent is the world
# outside the process: no file IO gem, no `system`, no sockets. A strategy
# describes itself; it does not go and do things. That is not a sandbox
# against a hostile author, it is the shape of the thing — the same reason a
# factor may not call `shift(-1)`.
MRuby::Build.new do |conf|
  conf.toolchain
  conf.gembox 'full-core'

  # 我们自己的扩展：JSON 生成。词汇表不在这里 —— 那是使用方的事，
  # 这个包只提供机器。
  conf.gem File.expand_path('mrbgems/mruby-json', __dir__)

  # Position-independent: the archive is linked into a Python extension
  # module, which is loaded as a shared object.
  conf.cc.flags << '-fPIC'
  conf.cc.flags << '-O2'

  conf.enable_debug

  # 跑 gem 自己的 Ruby 测试时才把 mrbtest 装上 —— 平时链进扩展的那份
  # 归档里不该有测试代码。
  conf.enable_test if ENV['JOHNNY_DSL_TEST']
end
