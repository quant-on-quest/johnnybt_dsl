# What kind of Ruby the DSL gets, and what compiler builds it.
#
# The full core: a strategy file is a program, so the language behind it is
# the whole one — Struct, Enumerable, Comparable, String formatting, Math,
# Time, exceptions with backtraces. What is deliberately absent is the world
# outside the process: no file IO gem, no `system`, no sockets. A strategy
# describes itself; it does not go and do things. That is not a sandbox
# against a hostile author, it is the shape of the thing — the same reason a
# factor may not call `shift(-1)`.
#
# The compiler is not guessed here. `build.rs` asks the `cc` crate what cargo
# is compiling C with for this target and passes it down, because that is the
# only answer that links: a machine can have a MinGW `gcc` on its path and an
# MSVC Rust target, and mruby's own guess would take the `gcc`.

# What build.rs decided.
toolchain = (ENV['JOHNNY_DSL_TOOLCHAIN'] || 'gcc').to_sym
compiler = ENV['JOHNNY_DSL_CC']
archiver = ENV['JOHNNY_DSL_AR']
# Set only when the target is not this machine.
cross = ENV['JOHNNY_DSL_CROSS']

# Point a build at the compiler cargo is using.
#
# The linker is only the compiler on the toolchains where the compiler *is*
# the linker driver. MSVC's is `link.exe`, and handing it `cl.exe` instead
# would break the one thing the host build exists for — compiling `mrbc`.
aim = lambda do |conf|
  conf.cc.command = compiler if compiler
  conf.linker.command = compiler if compiler && toolchain != :visualcpp
  conf.archiver.command = archiver if archiver
  # rake -m runs many cl.exe at once, and `/Zi` has them all write one
  # vc140.pdb: without `/FS` the second one in fails with C1041. The
  # stable release's toolchain file does not say it; this one does.
  conf.cc.flags << '/FS' if toolchain == :visualcpp
end

# Everything both builds share: the language, our two gems, and the flags
# that let the archive be linked into a shared object.
words = lambda do |conf|
  conf.gembox 'full-core'

  # Our own gems: JSON generation, and the DSL base class. Both are
  # infrastructure, not vocabulary — what a language *says* stays with
  # whoever defines it.
  conf.gem File.expand_path('mrbgems/mruby-json', __dir__)
  conf.gem File.expand_path('mrbgems/mruby-dsl', __dir__)

  # Position-independent: the archive is linked into a Python extension
  # module, which is loaded as a shared object. MSVC has no such flag —
  # everything it makes is relocatable.
  conf.cc.flags << '-fPIC' unless conf.cc.command.to_s =~ /cl(\.exe)?\z/
  conf.enable_debug

  # mrbtest goes in only when the gems' own Ruby tests are being run: the
  # archive that gets linked into the extension has no business carrying
  # test code.
  conf.enable_test if ENV['JOHNNY_DSL_TEST']
end

# The host build always exists, because `mrbc` — the bytecode compiler that
# turns our gems' Ruby into C — has to run on *this* machine.
MRuby::Build.new do |conf|
  if cross
    conf.toolchain
  else
    conf.toolchain toolchain
    aim.call(conf)
  end
  words.call(conf)
end

# Cross-compiling: one more build, for the target, borrowing the host's mrbc.
if cross
  MRuby::CrossBuild.new(cross) do |conf|
    conf.toolchain toolchain
    aim.call(conf)
    words.call(conf)
  end
end
