# JSON, spelled the way everybody spells it.
#
# Generation is in C (`src/json.c`): the escaping has to be exact and this
# runs on every call. Here it is only wired into the familiar names —
# `JSON.generate(x)` / `JSON.dump(x)` / `x.to_json`.
#
# Generation only, no parsing: what is wanted in this direction is "write
# the structure out", and reading it back is the host language's business.
# What is not done is not pretended.

module JSON
  # `generate` under the name Ruby's standard library uses.
  def self.dump(value)
    generate(value)
  end
end

class Object
  # This object as JSON. A type nothing else matches is written as its
  # `to_s`, quoted.
  def to_json
    JSON.generate(self)
  end
end
