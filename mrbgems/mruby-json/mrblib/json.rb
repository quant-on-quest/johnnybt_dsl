# JSON —— 标准的那套写法。
#
# 生成在 C 里（src/json.c）：转义要精确，而这是每次都要跑的一步。这里只把
# 它接成大家熟悉的样子 —— `JSON.generate(x)` / `JSON.dump(x)` / `x.to_json`。
#
# 只有生成，没有解析：这个方向上的需求是「把结构写出去」，而读回来的那一
# 侧是宿主语言的事（Python 有它自己的 json）。不做的事就不假装能做。

module JSON
  # `generate` 的另一个名字，和 Ruby 标准库对齐。
  def self.dump(value)
    generate(value)
  end
end

class Object
  # 这个对象的 JSON 写法。认不出来的类型按它的 `to_s` 当字符串写。
  def to_json
    JSON.generate(self)
  end
end
