##
# johnnybt DSL 的测试，跑在 mruby 自己的测试框架里。
#
# 这一层测的是**语言这一侧**：词落到 IR 的哪个键、块怎么收集、错误怎么收。
# Rust 那侧测「一次求值」整条路，Python 那侧测 IR 之后怎么绑模板 ——
# 三层各测各的，谁也不替谁。序列化是 mruby-json 自己的测试。

assert('一份策略的词落到 IR 的哪里') do
  JohnnyDSL.strategies.clear
  strategy "甲" do
    template :官方选股
    selector :周黎明
    count 10
    period "W", offsets: [0, 1]
  end
  ir = JohnnyDSL.strategies.last.to_ir

  assert_equal "甲", ir["name"]
  assert_equal "官方选股", ir["template"]
  assert_equal "周黎明", ir["selector"]
  assert_equal 10, ir["bindings"]["count"]
  assert_equal "W", ir["bindings"]["period"]
  assert_equal [0, 1], ir["bindings"]["offsets"]
end

assert('params 块一行一个阈值') do
  JohnnyDSL.strategies.clear
  strategy "乙" do
    params do
      短动量入围 100
      长动量分位下限 0.2
    end
  end
  params = JohnnyDSL.strategies.last.to_ir["params"]

  assert_equal 100, params["短动量入围"]
  assert_equal 0.2, params["长动量分位下限"]
end

assert('因子的参数与元数据是两半') do
  JohnnyDSL.strategies.clear
  strategy "丙" do
    factors do
      use "价量.动量", n: 5, as: :短动量, ascending: false, weight: 1.0
    end
  end
  entry = JohnnyDSL.strategies.last.to_ir["factors"].first

  assert_equal "价量.动量", entry["reference"]
  assert_equal({ "n" => 5 }, entry["params"])
  assert_equal "短动量", entry["meta"]["角色"]
  assert_equal false, entry["meta"]["ascending"]
  assert_equal 1.0, entry["meta"]["weight"]
end

assert('过滤条件的几种写法') do
  JohnnyDSL.strategies.clear
  strategy "丁" do
    filters do
      keep "规模.成交额均值", n: 5, at_least: 5000_0000
      keep "筹码.解套卖压", bottom_share: 0.8
      keep "成长.ROE", keep: "pct:<=0.5"
    end
  end
  conditions = JohnnyDSL.strategies.last.to_ir["filters"].map { |one| one["meta"]["keep"] }

  assert_equal "val:>=50000000", conditions[0]
  assert_equal "pct:<=0.8", conditions[1]
  assert_equal "pct:<=0.5", conditions[2]
end

assert('没写条件的过滤当场报错') do
  assert_raise(ArgumentError) do
    JohnnyDSL::FactorList.collect { keep "规模.成交额均值", n: 5 }
  end
end

assert('模板没有的词就当模板参数绑上去') do
  JohnnyDSL.strategies.clear
  strategy "戊" do
    厂商周期表 false
    min_bars 250
  end
  bindings = JohnnyDSL.strategies.last.to_ir["bindings"]

  assert_equal false, bindings["厂商周期表"]
  assert_equal 250, bindings["min_bars"]
end

assert('策略是程序：循环声明一族') do
  JohnnyDSL.strategies.clear
  [5, 10].each do |n|
    strategy "动量#{n}" do
      count n
    end
  end

  assert_equal ["动量5", "动量10"], JohnnyDSL.strategies.map(&:name)
  assert_equal [5, 10], JohnnyDSL.strategies.map { |one| one.to_ir["bindings"]["count"] }
end

assert('策略里抛出来的错进 failures，不打断整份文件') do
  JohnnyDSL.strategies.clear
  JohnnyDSL.failures.clear
  strategy "会抛的" do
    raise "我不干了"
  end

  assert_equal 0, JohnnyDSL.strategies.size
  assert_true JohnnyDSL.failures.first["message"].include?("我不干了")
end
