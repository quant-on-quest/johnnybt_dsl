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

  # Position-independent: the archive is linked into a Python extension
  # module, which is loaded as a shared object.
  conf.cc.flags << '-fPIC'
  conf.cc.flags << '-O2'

  conf.enable_debug
end
