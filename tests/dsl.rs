//! What the DSL evaluates to.
//!
//! The engine's contract is one string in, one JSON string out, so these
//! read the JSON as text: asserting on substrings keeps the tests from
//! carrying a JSON parser the crate itself does not need, and the shapes
//! being checked are small enough to name exactly.

use johnny_dsl::evaluate_source as evaluate;

/// Evaluate and return the IR, failing the test with Ruby's own message.
fn ir(source: &str) -> String {
    evaluate(source, "<test>").unwrap_or_else(|failure| panic!("DSL 没跑通：{failure}"))
}

#[test]
fn a_bare_strategy_carries_its_name() {
    let out = ir(r#"strategy "小市值·周黎明" do
end"#);

    assert!(out.contains(r#""name":"小市值·周黎明""#), "{out}");
}

#[test]
fn the_words_land_where_python_expects_them() {
    let out = ir(r#"
strategy "周黎明" do
  template :官方选股
  selector :周黎明
  count 10
  period "W", offsets: [0]

  params do
    短动量入围 100
    强势行业数 2
    长动量分位下限 0.2
  end
end
"#);

    assert!(out.contains(r#""template":"官方选股""#), "{out}");
    assert!(out.contains(r#""selector":"周黎明""#), "{out}");
    assert!(out.contains(r#""count":10"#), "{out}");
    assert!(out.contains(r#""period":"W""#), "{out}");
    assert!(out.contains(r#""offsets":[0]"#), "{out}");
    assert!(out.contains(r#""短动量入围":100"#), "{out}");
    assert!(out.contains(r#""长动量分位下限":0.2"#), "{out}");
}

#[test]
fn a_factor_splits_into_params_and_metadata() {
    let out = ir(r#"
strategy "带因子" do
  factors do
    use "price_volume.动量", n: 5, as: :短动量, ascending: false
  end
end
"#);

    assert!(out.contains(r#""reference":"price_volume.动量""#), "{out}");
    assert!(out.contains(r#""params":{"n":5}"#), "{out}");
    assert!(out.contains(r#""角色":"短动量""#), "{out}");
    assert!(out.contains(r#""ascending":false"#), "{out}");
}

#[test]
fn a_filter_spells_its_condition() {
    let out = ir(r#"
strategy "带过滤" do
  filters do
    keep "规模.成交额均值", n: 5, at_least: 5000_0000
  end
end
"#);

    assert!(out.contains(r#""keep":"val:>=50000000""#), "{out}");
}

#[test]
fn it_is_a_real_language() {
    // The whole point: a strategy that needs a loop writes a loop, and one
    // that needs a constant names it. Neither is expressible in YAML.
    let out = ir(r#"
窗口 = [5, 10, 20]

窗口.each do |n|
  strategy "动量#{n}" do
    count 10
    factors do
      use "price_volume.动量", n: n, ascending: false
    end
  end
end
"#);

    assert!(out.contains(r#""name":"动量5""#), "{out}");
    assert!(out.contains(r#""name":"动量20""#), "{out}");
    assert_eq!(out.matches(r#""name""#).count(), 3, "{out}");
}

#[test]
fn a_method_of_ones_own_is_just_ruby() {
    let out = ir(r#"
def 通用过滤(s)
  s.filters do
    keep "规模.成交额均值", n: 5, at_least: 5000_0000
  end
end

strategy "甲" do
  通用过滤(self)
end
"#);

    assert!(out.contains(r#""val:>=50000000""#), "{out}");
}

#[test]
fn a_mistake_comes_back_with_its_line() {
    let out = ir(r#"
strategy "坏的" do
  filters do
    keep "规模.成交额均值", n: 5
  end
end
"#);

    assert!(out.contains(r#""failures""#), "{out}");
    assert!(out.contains("过滤条件没写"), "{out}");
    assert!(out.contains("<test>"), "从回溯里应当看得见文件名：{out}");
}

#[test]
fn a_syntax_error_is_the_engines_own_error() {
    let failure = evaluate("strategy \"没写完\" do", "<test>").expect_err("这段语法上就不成立");

    assert!(format!("{failure}").contains("<test>"), "{failure}");
}

#[test]
fn every_evaluation_starts_clean() {
    // One interpreter per evaluation: a constant set by one file must not be
    // visible to the next, or two strategies would quietly differ by what
    // ran before them.
    ir("常量 = 1");
    let out = ir(r#"
strategy "第二份" do
  meta 见过常量: defined?(常量) ? true : false
end
"#);

    assert!(out.contains(r#""见过常量":false"#), "{out}");
}
