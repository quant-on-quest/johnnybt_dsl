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
