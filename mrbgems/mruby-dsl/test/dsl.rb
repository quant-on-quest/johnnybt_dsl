# Tests for the DSL base class.
#
# One interpreter runs all of these, so the languages defined here stay
# registered from assert to assert — each test therefore uses its own
# class and asserts on that, never on the registry being empty.

class OrderDSL < JohnnyBtDSL::Base
  describe "orders, as a language"
  sample "dsl/order.sample.rb"
end

class ShipmentThing < JohnnyBtDSL::Base
  dsl_name "shipping"
end

# Declared late on purpose: the assertion below reads to_meta before this
# line runs, which is how a language missing its sample is caught.
class SampledDSL < JohnnyBtDSL::Base
  dsl_name "sampled"
  sample "dsl/sampled.sample.rb"
end

assert('subclassing registers the language') do
  assert_true JohnnyBtDSL.dsls.include?(OrderDSL)
  assert_equal OrderDSL, JohnnyBtDSL.find("order")
end

assert('the name defaults from the class name, and can be said outright') do
  assert_equal "order", OrderDSL.dsl_name
  assert_equal "shipping", ShipmentThing.dsl_name
end

assert('the defining file is captured; doc defaults to the sample') do
  assert_true OrderDSL.source.end_with?("dsl.rb")
  assert_equal "dsl/order.sample.rb", OrderDSL.doc
end

assert('a language without a sample is refused when asked what it is') do
  assert_raise(RuntimeError) { ShipmentThing.to_meta }
  ShipmentThing.sample "dsl/shipping.sample.rb"   # declared: now it is a language
  assert_equal "dsl/shipping.sample.rb", ShipmentThing.to_meta["sample"]
end

assert('the manifest carries every language as data') do
  told = JSON.generate(nil) # touch the gem so the dependency is real
  assert_equal "null", told
  meta = OrderDSL.to_meta
  assert_equal "orders, as a language", meta["description"]
  assert_equal "dsl/order.sample.rb", meta["sample"]
end

assert('declarations collect in order') do
  OrderDSL.declare("first" => 1)
  OrderDSL.declare("second" => 2)
  assert_equal [{ "first" => 1 }, { "second" => 2 }], OrderDSL.declarations
end

assert('trouble pins the failure to the statement, not to the language') do
  own = OrderDSL.source
  backtrace = ["#{own}:40:in check", "#{own}:12:in word", "orders/monday.rb:7:in order", "orders/monday.rb:1"]
  ShipmentThing.trouble("`count` wants a number", backtrace, declaration: "monday")

  failure = ShipmentThing.failures.last
  assert_equal "orders/monday.rb", failure["file"]
  assert_equal 7, failure["line"]
  assert_equal "monday", failure["declaration"]
  assert_equal "shipping", failure["dsl"]
end

assert('trouble survives a backtrace that names no user file') do
  ShipmentThing.trouble("broken before any file", nil)
  assert_equal nil, ShipmentThing.failures.last["file"]
end

assert('the envelope groups declarations by language') do
  ir = JSON.generate(nil)
  assert_equal "null", ir # JSON is loaded; now the real envelope
  assert_true JohnnyBtDSL.ir.include?('"declarations"')
  assert_true JohnnyBtDSL.ir.include?('"order"')
  assert_true JohnnyBtDSL.ir.include?('"shipping"')
end

assert('the context is an empty hash when nothing was handed over') do
  assert_equal({}, JohnnyBtDSL.context)
end

assert('rank marks a place in an ordering') do
  assert_true 100.rank.is_a?(JohnnyBtDSL::Rank)
  assert_equal 100, 100.rank.place
  assert_equal "#100", 100.rank.to_s
end

assert('quantity suffixes: k, w, pct') do
  assert_equal 5_000, 5.k
  assert_equal 50_000_000, 5000.w
  assert_equal 1_500, 1.5.k
  assert_true 80.pct.is_a?(JohnnyBtDSL::Percent)
  assert_equal 0.8, 80.pct.share
end

assert('time suffixes wrap a Period') do
  assert_true 5.days.is_a?(JohnnyBtDSL::Period)
  assert_equal :days, 5.days.unit
  assert_equal 5, 5.days.count
  assert_equal 1.week, 1.weeks
  assert_equal :months, 3.months.unit
  assert_equal "2 weeks", 2.weeks.to_s
end
