# 跑我们这两个 gem 自己的测试。
#
#     build/host/bin/mruby mrbgems/run_tests.rb
#
# 不走 `rake test`：那个会把 mruby core 加 full-core 里每个 gem 的测试全
# 编进 mrbtest，为二十条断言付整套 mruby 测试的编译费。测试文件仍写成
# mrbtest 认的 `assert` 形式，所以谁想跑 `rake test` 一样跑得起来。

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

  raise "期待 #{expected.inspect}，得到 #{actual.inspect}"
end

def assert_true(value)
  raise "期待真，得到 #{value.inspect}" unless value
end

def assert_raise(kind)
  yield
  raise "期待抛 #{kind}，什么都没抛"
rescue StandardError => e
  raise "期待抛 #{kind}，抛的是 #{e.class}" unless e.is_a?(kind)
end

[
  "mrbgems/mruby-json/test/json.rb",
  "mrbgems/mruby-johnnybt/test/dsl.rb",
].each { |file| eval(File.read(file), nil, file) }

puts "#{$passed} 条通过，#{$failed.size} 条失败"
$failed.each { |one| puts "  #{one}" }
exit($failed.empty? ? 0 : 1)
