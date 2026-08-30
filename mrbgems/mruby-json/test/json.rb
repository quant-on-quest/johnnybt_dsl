##
# JSON 生成。

assert('JSON.generate 的基本类型') do
  assert_equal 'null', JSON.generate(nil)
  assert_equal 'true', JSON.generate(true)
  assert_equal 'false', JSON.generate(false)
  assert_equal '10', JSON.generate(10)
  assert_equal '0.2', JSON.generate(0.2)
  assert_equal '"甲"', JSON.generate("甲")
  assert_equal '"短动量"', JSON.generate(:短动量)
end

assert('JSON.dump 是 generate 的另一个名字') do
  assert_equal JSON.generate({ "a" => 1 }), JSON.dump({ "a" => 1 })
end

assert('#to_json 在每种东西上都在') do
  assert_equal '{"a":1}', { "a" => 1 }.to_json
  assert_equal '[1,2]', [1, 2].to_json
  assert_equal '"x"', "x".to_json
  assert_equal '3', 3.to_json
  assert_equal 'null', nil.to_json
  assert_equal 'true', true.to_json
end

assert('转义') do
  quote = 34.chr
  slash = 92.chr
  assert_equal quote + 'a' + slash + quote + 'b' + quote, ('a' + quote + 'b').to_json
  assert_equal quote + 'a' + slash + slash + 'b' + quote, ('a' + slash + 'b').to_json
  assert_equal quote + 'a' + slash + 'n' + 'b' + quote, "a\nb".to_json
  assert_equal quote + slash + 'u0001' + quote, 1.chr.to_json
end

assert('中文原样过去，不转成 uXXXX') do
  assert_equal '"小市值·周黎明"', "小市值·周黎明".to_json
end

assert('嵌套') do
  assert_equal '{"n":[1,{"m":null}]}', { "n" => [1, { "m" => nil }] }.to_json
end

assert('键一律写成字符串') do
  assert_equal '{"1":"x"}', { 1 => "x" }.to_json
  assert_equal '{"甲":1}', { 甲: 1 }.to_json
end

assert('自己引用自己的结构被拒绝，而不是把栈跑穿') do
  ring = []
  ring << ring
  assert_raise(ArgumentError) { ring.to_json }
end
