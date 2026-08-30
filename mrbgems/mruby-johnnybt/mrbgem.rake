MRuby::Gem::Specification.new('mruby-johnnybt') do |spec|
  spec.license = 'MIT'
  spec.author  = 'MJ'
  spec.summary = 'johnnybt 策略 DSL：词汇表与 IR 序列化'

  # 词汇表在 mrblib 里，构建时就编成字节码进 libmruby.a —— 每次求值不必
  # 再解析一遍，也就不存在「词汇表在运行时语法错」这种事。
  #
  # 序列化不在这里：那是 mruby-json 的事，和策略无关。
  spec.add_dependency 'mruby-json', :path => File.expand_path('../mruby-json', __dir__)
end
