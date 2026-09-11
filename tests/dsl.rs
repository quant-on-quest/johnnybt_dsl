//! What the engine does: run Ruby, hand back a string.
//!
//! There is no vocabulary here to test — the engine has none. What these
//! check is the machine: chunks run in order, bytecode is source's equal,
//! errors name their chunk, and one evaluation cannot see another's world.

use johnnybt_dsl::{compile_source, run_chunks, Chunk};

/// Run one source chunk and return the answer.
fn answer(source: &str, answer: &str) -> String {
    run_chunks(&[Chunk::Source(source, "<test>")], answer, None).unwrap_or_else(|failure| panic!("{failure}"))
}

#[test]
fn it_runs_ruby_and_returns_the_answer() {
    assert_eq!(answer("x = 6 * 7", "x.to_s"), "42");
}

#[test]
fn the_whole_language_is_there() {
    let out = answer(
        r#"
        Point = Struct.new(:x, :y)
        points = [1, 2, 3].map { |n| Point.new(n, n * n) }
        total = points.reduce(0) { |sum, one| sum + one.y }
        "#,
        "total.to_s",
    );

    assert_eq!(out, "14");
}

#[test]
fn chunks_run_in_order_and_see_each_other() {
    let out = run_chunks(
        &[
            Chunk::Source("def twice(n) = n * 2", "<first>"),
            Chunk::Source("answer = twice(21)", "<second>"),
        ],
        "answer.to_s",
        None,
    )
    .expect("both chunks run");

    assert_eq!(out, "42");
}

#[test]
fn bytecode_is_sources_equal() {
    let bytecode = compile_source("def twice(n) = n * 2", "<words>").expect("it compiles");
    let out = run_chunks(
        &[
            Chunk::Bytecode(&bytecode, "<words>"),
            Chunk::Source("answer = twice(21)", "<file>"),
        ],
        "answer.to_s",
        None,
    )
    .expect("bytecode loads");

    assert_eq!(out, "42");
}

#[test]
fn the_caller_defines_what_the_answer_looks_like() {
    // The engine has no idea what a strategy is; this shape is the test's.
    let words = r#"
      module Mine
        def self.seen = @seen ||= []
        def self.result = seen.to_json
      end

      def note(what) = Mine.seen << what
    "#;
    let out = run_chunks(
        &[Chunk::Source(words, "<words>"), Chunk::Source(r#"note "first"; note "second""#, "<file>")],
        "Mine.result",
        None,
    )
    .expect("it runs");

    assert_eq!(out, r#"["first","second"]"#);
}

#[test]
fn a_failing_chunk_names_itself() {
    let failure = run_chunks(&[Chunk::Source("raise 'nope'", "strategy.rb")], "nil.to_s", None).expect_err("it raises");

    assert!(format!("{failure}").contains("strategy.rb"), "{failure}");
    assert!(format!("{failure}").contains("nope"), "{failure}");
}

#[test]
fn a_syntax_error_names_its_chunk_too() {
    let failure = run_chunks(&[Chunk::Source("def unfinished(", "broken.rb")], "nil.to_s", None).expect_err("it is broken");

    assert!(format!("{failure}").contains("broken.rb"), "{failure}");
}

#[test]
fn source_that_does_not_compile_is_refused_at_compile_time() {
    let failure = compile_source("def unfinished(", "broken.rb").expect_err("it is broken");

    assert!(format!("{failure}").contains("broken.rb"), "{failure}");
}

#[test]
fn every_run_starts_clean() {
    // One interpreter per run: what one file defined must not be visible to
    // the next, or two runs would quietly differ by what ran before them.
    answer("SEEN = 1", "nil.to_s");

    assert_eq!(answer("", "defined?(SEEN) ? 'saw it' : 'clean'"), "clean");
}

#[test]
fn the_dsl_base_ships_with_the_machine() {
    // The one convention every hosted language shares: subclass
    // JohnnyBtDSL::Base, declare, and the envelope reads it back.
    let out = run_chunks(
        &[Chunk::Source(
            r#"
            class ProbeDSL < JohnnyBtDSL::Base
              describe "a probe"
              sample "probe.rb"
            end
            ProbeDSL.declare({"n" => 1})
            "#,
            "probe.rb",
        )],
        "JohnnyBtDSL.ir",
        None,
    )
    .expect("the base is in the VM");

    assert!(out.contains(r#""probe""#), "{out}");
    assert!(out.contains(r#""declarations""#), "{out}");
}

#[test]
fn a_failure_is_pinned_to_the_statement_even_through_the_cores_own_ruby() {
    // A raise from inside a block handed to `Hash#each` has the core's own
    // `mrblib/hash.rb` on top of the stack: written in Ruby since mruby
    // 4.1, and reporting itself with no leading directory. The failure
    // still belongs to the user's line, not to the interpreter's.
    let out = run_chunks(
        &[
            Chunk::Source(
                r##"
                class ProbeDSL < JohnnyBtDSL::Base
                  describe "a probe"
                  sample "probe.rb"
                  def self.check(pairs)
                    pairs.each { |name, value| raise ArgumentError, "#{name} is not a number" unless value.is_a?(Numeric) }
                  rescue => e
                    trouble(e.message, e.backtrace, declaration: "probe")
                  end
                end
                "##,
                "words.rb",
            ),
            Chunk::Source("\n\nProbeDSL.check({\"cut\" => {}})\n", "strategy.rb"),
        ],
        "JohnnyBtDSL.ir",
        None,
    )
    .expect("the failure is collected, not raised");

    assert!(out.contains(r#""file":"strategy.rb""#), "{out}");
    assert!(out.contains(r#""line":3"#), "{out}");
}

#[test]
fn json_comes_from_the_gem_we_ship() {
    // The one thing the engine does bring: JSON generation, because a
    // string is what crosses back and the escaping has to be exact.
    assert_eq!(answer("", r#"{"name" => "a", "n" => [1, nil]}.to_json"#), r#"{"name":"a","n":[1,null]}"#);
}

#[test]
fn a_context_reaches_the_program_as_a_global() {
    // The host hands a JSON blob; the program reads `$johnny_context`.
    // The engine neither parses nor cares what is in it.
    let out = run_chunks(
        &[Chunk::Source("told = JSON.parse($johnny_context)", "<file>")],
        r#"told["名字"]"#,
        Some(r#"{"名字":"甲","n":[1,2]}"#),
    )
    .expect("the context is there");

    assert_eq!(out, "甲");
}

#[test]
fn without_a_context_the_global_is_nil() {
    let out = run_chunks(&[], "$johnny_context.inspect", None).expect("it runs");

    assert_eq!(out, "nil");
}
