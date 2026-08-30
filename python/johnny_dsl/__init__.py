"""An mruby virtual machine, ready to use.

The package provides the machine and nothing else: run some chunks of Ruby
in order, then one expression, and get back the string it produced. What
those chunks define is yours — words, modules, whatever shape of answer you
want. A language baked in here would make one library serve one project.

Installing needs neither a C compiler nor Ruby: the wheel ships mruby
already compiled in.

    from johnny_dsl import compile, run

    words = compile(open("dsl.rb").read(), "dsl.rb")   # once per process
    answer = run([(words, "dsl.rb"), (source, path)], answer="MyDSL.result")

A chunk is Ruby source (`str`) or bytecode (`bytes`) — bytecode does not
belong to the interpreter that ran it, so a chunk you run again and again is
compiled once and loaded after.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from johnny_dsl._lib import compile as _compile
from johnny_dsl._lib import run as _run

if TYPE_CHECKING:
    from collections.abc import Sequence

__all__ = ["DSLError", "compile", "run", "run_file"]


class DSLError(Exception):
    """Some Ruby did not run — the message carries its own error."""


def compile(source: str, name: str = "<chunk>") -> bytes:  # noqa: A001 - it compiles; the builtin is not what a caller means here
    """Compile Ruby source to bytecode.

    Args:
        source: The Ruby to compile.
        name: What to call it in a backtrace.

    Returns:
        The bytecode, loadable into any later interpreter.

    Raises:
        DSLError: If it does not compile.
    """
    try:
        return _compile(source, name)
    except ValueError as error:
        raise DSLError(str(error)) from error


def run(
    chunks: Sequence[tuple[str | bytes, str] | str | bytes],
    answer: str = "nil.to_s",
    context: str | None = None,
) -> str:
    """Run chunks of Ruby in order, then an expression, and return its string.

    Args:
        chunks: What to run, in order. Each is `(source_or_bytecode, name)`,
            or a bare `str`/`bytes` when the name does not matter.
        answer: The expression run last, whose string value comes back.
        context: Handed to the program as the global `$johnny_context`
            before anything runs — a JSON blob by convention (the shipped
            base class parses it lazily as `JohnnyDSL.context`).

    Returns:
        Whatever the answer produced.

    Raises:
        DSLError: If a chunk does not run.
    """
    try:
        return _run(list(chunks), answer, context)
    except ValueError as error:
        raise DSLError(str(error)) from error


def run_file(
    path: str,
    before: Sequence[tuple[str | bytes, str] | str | bytes] = (),
    answer: str = "nil.to_s",
    context: str | None = None,
) -> str:
    """Run a file, after whatever chunks come before it.

    Args:
        path: The file to read and run.
        before: Chunks to run first — the words the file is written in,
            typically.
        answer: The expression run last.
        context: Handed to the program as `$johnny_context`.

    Returns:
        Whatever the answer produced.

    Raises:
        DSLError: If it does not run.
        OSError: If the file cannot be read.
    """
    from pathlib import Path

    source = Path(path).read_text(encoding="utf-8")
    return run([*before, (source, str(path))], answer, context)
