##
# JSON generation.

assert('JSON.generate on the basic types') do
  assert_equal 'null', JSON.generate(nil)
  assert_equal 'true', JSON.generate(true)
  assert_equal 'false', JSON.generate(false)
  assert_equal '10', JSON.generate(10)
  assert_equal '0.2', JSON.generate(0.2)
  assert_equal '"a"', JSON.generate("a")
  assert_equal '"name"', JSON.generate(:name)
end

assert('JSON.dump is generate under another name') do
  assert_equal JSON.generate({ "a" => 1 }), JSON.dump({ "a" => 1 })
end

assert('#to_json is on everything') do
  assert_equal '{"a":1}', { "a" => 1 }.to_json
  assert_equal '[1,2]', [1, 2].to_json
  assert_equal '"x"', "x".to_json
  assert_equal '3', 3.to_json
  assert_equal 'null', nil.to_json
  assert_equal 'true', true.to_json
end

assert('escaping') do
  quote = 34.chr
  slash = 92.chr
  assert_equal quote + 'a' + slash + quote + 'b' + quote, ('a' + quote + 'b').to_json
  assert_equal quote + 'a' + slash + slash + 'b' + quote, ('a' + slash + 'b').to_json
  assert_equal quote + 'a' + slash + 'n' + 'b' + quote, "a\nb".to_json
  assert_equal quote + slash + 'u0001' + quote, 1.chr.to_json
end

assert('non-ASCII text passes through as UTF-8, not as uXXXX') do
  assert_equal '"小市值·周黎明"', "小市值·周黎明".to_json
end

assert('nesting') do
  assert_equal '{"n":[1,{"m":null}]}', { "n" => [1, { "m" => nil }] }.to_json
end

assert('keys are always written as strings') do
  assert_equal '{"1":"x"}', { 1 => "x" }.to_json
  assert_equal '{"name":1}', { name: 1 }.to_json
end

assert('a structure that contains itself is refused, not run off the stack') do
  ring = []
  ring << ring
  assert_raise(ArgumentError) { ring.to_json }
end

assert('JSON.parse on the basic types') do
  assert_equal nil, JSON.parse("null")
  assert_equal true, JSON.parse("true")
  assert_equal(-12, JSON.parse("-12"))
  assert_equal 2.5, JSON.parse("2.5")
  assert_equal "a", JSON.parse('"a"')
  assert_equal [1, 2], JSON.parse("[1, 2]")
  assert_equal({ "k" => 1 }, JSON.parse('{"k": 1}'))
end

assert('JSON.parse decodes escapes, UTF-8 passes through') do
  assert_equal "a\"b\\c\nd", JSON.parse('"a\"b\\\\c\nd"')
  assert_equal "中文😀", JSON.parse('"中文😀"')
  assert_equal "😀", JSON.parse('"😀"')
end

assert('JSON.parse round-trips what JSON.generate wrote') do
  value = { "名字" => "甲", "n" => [1, 2.5, nil, true, false], "嵌" => { "套" => [] } }
  assert_equal value, JSON.parse(JSON.generate(value))
end

assert('JSON.parse refuses garbage with the byte offset') do
  assert_raise(ArgumentError) { JSON.parse("{bad}") }
  assert_raise(ArgumentError) { JSON.parse('"x"y') }
  assert_raise(ArgumentError) { JSON.parse("[1,") }
  assert_raise(ArgumentError) { JSON.parse('"\ud83d"') }
end

assert('JSON.parse keeps integers integral when they fit') do
  assert_equal Integer, JSON.parse("9007199254740993").class
  assert_equal Float, JSON.parse("1e400").class
end
