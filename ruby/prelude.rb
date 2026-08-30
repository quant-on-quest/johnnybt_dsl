# 策略 DSL 的词汇表。
#
# 这是**真的 Ruby**：循环、常量、方法、模块、Struct、异常，全都在。一份策略
# 不再是「一张被 YAML 语法勉强撑住的表」，而是一段能表达自己的程序 ——
# 需要扫十个窗口就写个 each，需要一组策略共用一套过滤就写个方法，
# 需要按条件分叉就写 if。这正是 YAML 和「用 Python 硬凑 DSL」都给不了的东西。
#
# 与 Python 之间只有一样东西过界：一段 JSON IR。DSL 怎么写是 Ruby 的事，
# IR 怎么解释是 Python 的事，两边谁都不必迁就谁的语法。

module JohnnyDSL
  # 过界的那段 JSON 自己写 —— mruby 不带 JSON，而我们要序列化的东西只有
  # 五种：映射、数组、字符串、数、真假空。为这点事引一个 gem 不值。
  module JSON
    def self.dump(value)
      case value
      when nil then "null"
      when true then "true"
      when false then "false"
      when Integer then value.to_s
      when Float then value.to_s
      when Symbol then quote(value.to_s)
      when String then quote(value)
      when Array then "[" + value.map { |item| dump(item) }.join(",") + "]"
      when Hash then "{" + value.map { |key, item| "#{quote(key.to_s)}:#{dump(item)}" }.join(",") + "}"
      else quote(value.to_s)
      end
    end

    ESCAPES = {
      "\"" => "\\\"", "\\" => "\\\\", "\n" => "\\n", "\t" => "\\t",
      "\r" => "\\r", "\b" => "\\b", "\f" => "\\f"
    }.freeze

    def self.quote(text)
      out = "\""
      text.each_char do |char|
        escaped = ESCAPES[char]
        out << if escaped
                 escaped
               elsif char.ord < 0x20
                 format("\\u%04x", char.ord)
               else
                 char
               end
      end
      out << "\""
    end
  end

  # 收集起来的策略，按声明顺序。
  def self.strategies
    @strategies ||= []
  end

  # 出错时记下来 —— 抛给 Rust 会丢掉行号，写进 IR 才能带着回溯回到 Python。
  def self.failures
    @failures ||= []
  end

  def self.add(strategy)
    strategies << strategy
  end

  def self.fail!(message, backtrace)
    failures << { "message" => message, "backtrace" => backtrace || [] }
  end

  # 最终交给 Rust 的那段 JSON。
  def self.__ir__
    JSON.dump({
      "version" => 1,
      "strategies" => strategies.map(&:to_ir),
      "failures" => failures,
    })
  end

  # 一份策略声明。方法名就是 DSL 的词，每个词只做一件事：把值记下来。
  class Strategy
    RESERVED = %w[template selector meta start end].freeze

    attr_reader :name

    def initialize(name)
      @name = name
      @bindings = {}
      @meta = {}
      @factors = []
      @filters = []
      @params = {}
      @start = nil
      @finish = nil
    end

    # 用哪个模板 —— 对应 Python 那边 templates/<名字>.py。
    def template(name = nil)
      return @template if name.nil?

      @template = name.to_s
    end

    # 换掉「怎么打分」那一步：选股/<名字>.py。
    def selector(name = nil)
      return @selector if name.nil?

      @selector = name.to_s
    end

    # 选股算法自己的超参数。块里每一行是「名字 值」。
    def params(&block)
      return @params if block.nil?

      @params.merge!(Collector.collect(&block))
      @params
    end

    # 选几只。整数是只数，(0,1) 的小数是候选池的占比。
    def count(value = nil)
      value.nil? ? @bindings["count"] : @bindings["count"] = value
    end

    # 持仓周期，以及铺满哪几个 offset。
    def period(value = nil, offsets: nil)
      return @bindings["period"] if value.nil? && offsets.nil?

      @bindings["period"] = value.to_s unless value.nil?
      @bindings["offsets"] = offsets.to_a unless offsets.nil?
    end

    def offsets(*values)
      @bindings["offsets"] = values.flatten
    end

    # 回测窗口。策略自己的，不写就用项目配置里的。
    def window(from: nil, to: nil)
      @start = from.to_s unless from.nil?
      @finish = to.to_s unless to.nil?
    end

    # 开发者自定义的键袋：作者、标签、给任务读的开关。
    def meta(pairs = nil)
      return @meta if pairs.nil?

      @meta.merge!(stringify(pairs))
    end

    # 排序用的因子。
    def factors(&block)
      @factors.concat(FactorList.collect(&block)) if block
      @factors
    end

    # 前置过滤用的因子 —— 每条带自己的 keep 条件。
    def filters(&block)
      @filters.concat(FactorList.collect(&block)) if block
      @filters
    end

    # 模板的其它参数，直接按名字绑：`bind 名字: 值`。
    def bind(pairs)
      @bindings.merge!(stringify(pairs))
    end

    # DSL 里没有的词就当模板参数处理 —— 模板签名才是那份权威清单，
    # 这里再抄一遍只会漂移。拼错的键在 Python 那边按模板签名报错。
    def method_missing(name, *args)
      spelled = name.to_s
      if spelled.end_with?("=")
        @bindings[spelled.chomp("=")] = args.first
      elsif args.length == 1
        @bindings[spelled] = args.first
      elsif args.empty?
        @bindings[spelled]
      else
        @bindings[spelled] = args
      end
    end

    def respond_to_missing?(_name, _private = false)
      true
    end

    def to_ir
      {
        "name" => @name,
        "template" => @template,
        "selector" => @selector,
        "bindings" => @bindings,
        "params" => @params,
        "factors" => @factors,
        "filters" => @filters,
        "meta" => @meta,
        "start" => @start,
        "end" => @finish,
      }
    end

    private

    def stringify(pairs)
      out = {}
      pairs.each { |key, value| out[key.to_s] = value }
      out
    end
  end

  # `params do ... end` 里的收集器：一行一个「名字 值」。
  class Collector
    def self.collect(&block)
      collector = new
      collector.instance_eval(&block)
      collector.__values__
    end

    def initialize
      @values = {}
    end

    def __values__
      @values
    end

    def method_missing(name, *args)
      return @values[name.to_s] if args.empty?

      @values[name.to_s] = args.length == 1 ? args.first : args
    end

    def respond_to_missing?(_name, _private = false)
      true
    end
  end

  # `factors do ... end` / `filters do ... end` 里的收集器。
  class FactorList
    def self.collect(&block)
      list = new
      list.instance_eval(&block)
      list.__entries__
    end

    def initialize
      @entries = []
    end

    def __entries__
      @entries
    end

    # 一个因子：引用 + 它自己的参数 + 给 stage 读的元数据。
    #
    #   use "price_volume.动量", n: 5, as: :短动量, ascending: false
    #   keep "规模.成交额均值", n: 5, at_least: 5000_0000
    #
    # `as:` / `ascending:` / `weight:` / `keep:` 是元数据，其余键是因子参数
    # —— 和 Python 那边 Use 的两半一一对应。
    def use(reference, **options)
      @entries << entry(reference, options)
    end

    def keep(reference, **options)
      condition = options.delete(:keep)
      condition ||= "val:>=#{format_number(options.delete(:at_least))}" if options.key?(:at_least)
      condition ||= "val:<=#{format_number(options.delete(:at_most))}" if options.key?(:at_most)
      condition ||= "pct:>=#{options.delete(:top_share)}" if options.key?(:top_share)
      condition ||= "pct:<=#{options.delete(:bottom_share)}" if options.key?(:bottom_share)
      raise ArgumentError, "#{reference} 的过滤条件没写：keep:/at_least:/at_most:/top_share:/bottom_share: 挑一个" if condition.nil?

      @entries << entry(reference, options.merge(keep: condition))
    end

    private

    META = %i[as ascending weight keep].freeze

    def entry(reference, options)
      params = {}
      meta = {}
      options.each do |key, value|
        if META.include?(key)
          name = key == :as ? "角色" : key.to_s
          meta[name] = value.is_a?(Symbol) ? value.to_s : value
        else
          params[key.to_s] = value
        end
      end
      { "reference" => reference.to_s, "params" => params, "meta" => meta }
    end

    def format_number(value)
      return value if value.nil?
      value.to_i == value ? value.to_i.to_s : value.to_s
    end
  end
end

# 顶层的那个词。一份文件可以声明好几个策略 —— 一次扫十组参数就是十份。
def strategy(name, &block)
  declared = JohnnyDSL::Strategy.new(name.to_s)
  declared.instance_eval(&block)
  JohnnyDSL.add(declared)
  declared
rescue StandardError => e
  JohnnyDSL.fail!("#{name}: #{e.class}: #{e.message}", e.backtrace)
  nil
end
