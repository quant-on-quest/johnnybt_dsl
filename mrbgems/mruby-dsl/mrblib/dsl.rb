# A base for the languages hosted on this virtual machine.
#
# The engine runs Ruby and hands back a string; what a language *is* stays
# with whoever defines it. This gem is the one convention every language
# shares: how a DSL announces itself, how it hands over what its files
# declared, and how it reports **which statement** went wrong — the whole
# point of writing a DSL instead of a config format.
#
#     class OrderDSL < JohnnyDSL::Base
#       describe "orders, as a language"
#       sample   "dsl/order.sample.rb"
#     end
#
# Subclassing registers the language. The subclass collects with `declare`
# and reports with `trouble`; a host reads everything back through one
# envelope, `JohnnyDSL.ir`:
#
#     {"version": 1,
#      "dsls":         [{"name", "description", "doc", "sample", "source"}],
#      "declarations": {"<dsl name>": [...]},
#      "failures":     [{"dsl", "declaration", "message", "file", "line",
#                        "backtrace"}]}
#
# `JohnnyDSL.manifest` is the same without declarations — for tooling that
# only asks what languages exist and where their docs live.

module JohnnyDSL
  VERSION = 1

  class << self
    # What the host handed this run, parsed lazily from the global the
    # engine set. `{}` when nothing was handed — words can always index it.
    #
    # By convention the host of a project puts the project's configuration
    # under "config" and what else its languages need beside it; the keys
    # are the host's contract, not this gem's.
    def context
      @context ||= $johnny_context ? JSON.parse($johnny_context) : {}
    end

    # Every language defined so far, in definition order.
    def dsls
      @dsls ||= []
    end

    # Find a language by its name, or nil.
    def find(name)
      dsls.find { |dsl| dsl.dsl_name == name.to_s }
    end

    # What languages exist and where their docs live, as JSON.
    def manifest
      { "version" => VERSION, "dsls" => dsls.map(&:to_meta) }.to_json
    end

    # The envelope a host reads after running some files, as JSON.
    def ir
      declared = {}
      dsls.each { |dsl| declared[dsl.dsl_name] = dsl.declarations }
      {
        "version" => VERSION,
        "dsls" => dsls.map(&:to_meta),
        "declarations" => declared,
        "failures" => dsls.flat_map(&:failures),
      }.to_json
    end
  end

  # Derive from this to define a language.
  #
  # The class-level macros say what the language is; `declare` and
  # `trouble` are what its words call while a file runs. Everything else —
  # the words themselves, how they collect, what a declaration looks like
  # inside — belongs to the subclass.
  class Base
    class << self
      # The file that defined this language, captured at subclassing.
      attr_reader :source

      def inherited(sub)
        super
        # The frame under `inherited` is the `class X < Base` line itself,
        # which is how `source` (and the default for `doc`) knows the file
        # without anyone spelling it.
        sub.instance_variable_set(:@source, JohnnyDSL::Base.file_of(caller.first))
        JohnnyDSL.dsls << sub
      end

      # The language's name. Defaults to the class name without its DSL
      # suffix, lowercased: OrderDSL -> "order".
      def dsl_name(value = nil)
        @dsl_name = value.to_s unless value.nil?
        @dsl_name || (name || "").sub(/DSL\z/, "").downcase
      end

      # One line on what this language declares.
      def describe(text = nil)
        @describe = text unless text.nil?
        @describe || ""
      end

      # Where to read how to write this language. Defaults to the sample:
      # a runnable file with the rules in its comments is the best thing
      # to hand somebody about to write one, and it cannot drift from what
      # runs. Declare `doc` to point somewhere else entirely.
      def doc(path = nil)
        @doc = path unless path.nil?
        @doc || sample
      end

      # A working file to copy from — **every language declares one**.
      #
      # It is the documentation: `doc` defaults to it, and a file that
      # runs cannot describe a language that does not. A language without
      # one is refused when a host asks what it is.
      def sample(path = nil)
        @sample = path unless path.nil?
        @sample
      end

      # This language as data, for the manifest.
      #
      # Raises when the language declares no sample: a language nobody can
      # be shown how to write is not finished, and the gap surfaces here,
      # where its author is looking.
      def to_meta
        if sample.nil?
          raise "#{name}: 没有声明 sample —— 每门语言都要给一份能跑的范本" \
                "（`sample File.expand_path(\"examples/<名字>.rb\", File.dirname(__FILE__))`），" \
                "它就是这门语言的文档。"
        end
        {
          "name" => dsl_name,
          "description" => describe,
          "doc" => doc,
          "sample" => sample,
          "source" => source,
        }
      end

      # What this language's files declared, in order.
      def declarations
        @declarations ||= []
      end

      # What went wrong, one entry per failing statement.
      def failures
        @failures ||= []
      end

      # Collect one declaration.
      def declare(declaration)
        declarations << declaration
        declaration
      end

      # Collect one failure, pinned to the statement that raised it.
      #
      # The backtrace names the language's own frames too; the statement is
      # the first frame in nobody's implementation — which is why languages
      # must not parse backtraces themselves.
      def trouble(message, backtrace = nil, declaration: nil)
        frames = (backtrace || []).map(&:to_s)
        own = JohnnyDSL.dsls.map(&:source).compact
        # Skip the languages' own frames AND the interpreter's built-in
        # Ruby (anything under an mrblib/ - core or a gem's): a raise
        # inside `each`'s block otherwise pins the failure to hash.rb.
        spot = frames.find do |frame|
          !frame.include?("/mrblib/") && own.none? { |file| frame.start_with?("#{file}:") }
        end
        file, line = JohnnyDSL::Base.place_of(spot)
        failures << {
          "dsl" => dsl_name,
          "declaration" => declaration,
          "message" => message.to_s,
          "file" => file,
          "line" => line,
          "backtrace" => frames,
        }
        nil
      end

      # The file part of one backtrace frame.
      def file_of(frame)
        JohnnyDSL::Base.place_of(frame).first
      end

      # The file and line of one backtrace frame, either possibly nil.
      def place_of(frame)
        head = frame.to_s.split(":in ").first.to_s
        if head =~ /\A(.+):(\d+)\z/
          [::Regexp.last_match(1), ::Regexp.last_match(2).to_i]
        else
          [head.empty? ? nil : head, nil]
        end
      end
    end
  end
end


# Quantity suffixes for any hosted language: `5000.w >= threshold` reads
# the way people write numbers, and `80.pct` marks a share so a language
# can tell a percentile condition from an absolute one by type.
class Numeric
  # Thousands: 5.k == 5_000.
  def k
    self * 1_000
  end

  # Ten-thousands (the CJK 万, the unit A-share money is quoted in):
  # 5000.w == 50_000_000.
  def w
    self * 10_000
  end

  # A share of a whole: 80.pct wraps 0.8 as a JohnnyDSL::Percent, a
  # distinct type comparisons can dispatch on.
  def pct
    JohnnyDSL::Percent.new(self / 100.0)
  end

  # Spans of time: `period 5.days` reads the way people speak. Each wraps
  # the count in a JohnnyDSL::Period; what a "day" means (trading day?
  # calendar week?) is the hosting language's business, not this type's.
  def days
    JohnnyDSL::Period.new(:days, self)
  end
  alias day days

  def weeks
    JohnnyDSL::Period.new(:weeks, self)
  end
  alias week weeks

  def months
    JohnnyDSL::Period.new(:months, self)
  end
  alias month months
end

module JohnnyDSL
  # A share of a whole, carried as its own type.
  #
  # `80.pct` is not the number 0.8: a language that sees a Percent on the
  # right of a comparison knows the author meant a percentile cut, not an
  # absolute threshold, and can emit the right condition without a second
  # keyword.
  class Percent
    attr_reader :share

    def initialize(share)
      @share = share
    end

    def ==(other)
      other.is_a?(Percent) && other.share == @share
    end

    def to_f
      @share.to_f
    end

    def to_s
      "#{(@share * 100)}%"
    end
  end

  # A span of time: a unit (:days, :weeks, :months) and a count.
  #
  # `5.days` carries no calendar of its own — the hosting language decides
  # whether a day is a trading day and what a week anchors to, exactly as
  # it decides what a factor reference resolves to.
  class Period
    attr_reader :unit, :count

    def initialize(unit, count)
      @unit = unit
      @count = count
    end

    def ==(other)
      other.is_a?(Period) && other.unit == @unit && other.count == @count
    end

    def to_s
      "#{@count} #{@unit}"
    end
  end
end
