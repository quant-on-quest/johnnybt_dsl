MRuby::Gem::Specification.new('mruby-dsl') do |spec|
  spec.license = 'MIT'
  spec.author = 'MJ'
  spec.summary = 'A base class for DSLs: registration, declarations, failures pinned to statements'
  spec.add_dependency 'mruby-json', path: File.expand_path('../mruby-json', __dir__)
end
