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

      # Where to read how to write this language. Defaults to the file the
      # language is defined in — its comments are the documentation.
      def doc(path = nil)
        @doc = path unless path.nil?
        @doc || source
      end

      # A working file to copy from.
      def sample(path = nil)
        @sample = path unless path.nil?
        @sample
      end

      # This language as data, for the manifest.
      def to_meta
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

    def to_s
      "#{(@share * 100)}%"
    end
  end
end
