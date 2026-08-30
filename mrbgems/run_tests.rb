# Run this gem's own tests.
#
#     build/host/bin/mruby mrbgems/run_tests.rb
#
# Not through `rake test`: that compiles mruby's core suite and every
# full-core gem's tests into mrbtest — the whole of mruby's test build for
# twenty assertions here. The test files are still written in the `assert`
# form mrbtest knows, so `rake test` runs them too.

$passed = 0
$failed = []

def assert(name)
  yield
  $passed += 1
rescue StandardError => e
  $failed << "#{name}: #{e.class}: #{e.message}"
end

def assert_equal(expected, actual)
  return if expected == actual

  raise "expected #{expected.inspect}, got #{actual.inspect}"
end

def assert_true(value)
  raise "expected a true value, got #{value.inspect}" unless value
end

def assert_raise(kind)
  yield
  raise "expected #{kind}, nothing was raised"
rescue StandardError => e
  raise "expected #{kind}, got #{e.class}" unless e.is_a?(kind)
end

[
  "mrbgems/mruby-json/test/json.rb",
  "mrbgems/mruby-dsl/test/dsl.rb",
].each { |file| eval(File.read(file), nil, file) }

puts "#{$passed} passed, #{$failed.size} failed"
$failed.each { |one| puts "  #{one}" }
exit($failed.empty? ? 0 : 1)
