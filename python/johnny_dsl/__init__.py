"""策略 DSL：一门真正的语言写策略，一段 JSON IR 过界。

一份策略是一段程序，所以写它的该是一门语言。mruby 跑在这个扩展里面，DSL
的词汇表用 Ruby 写（`ruby/prelude.rb`），回到 Python 的只有一样东西：IR。
两边谁都不必迁就谁的语法 —— 这正是 YAML 和「用 Python 硬凑 DSL」一直在
付的代价。

    from johnny_dsl import evaluate_file
    ir = evaluate_file("strategies/小市值/周黎明.rb")
"""

from __future__ import annotations

import json
from typing import TYPE_CHECKING, Any

from johnny_dsl._lib import evaluate as _evaluate
from johnny_dsl._lib import evaluate_file as _evaluate_file

if TYPE_CHECKING:
    from pathlib import Path

__all__ = ["DSLError", "evaluate", "evaluate_file"]


class DSLError(Exception):
    """一份 DSL 跑不通 —— 消息里带着 Ruby 那边的错误和回溯。"""


def evaluate(source: str, name: str = "<dsl>") -> dict[str, Any]:
    """Evaluate DSL source and return its IR.

    Args:
        source: The strategy source.
        name: What to call it in a backtrace.

    Returns:
        The IR: `{"version", "strategies": [...], "failures": [...]}`.

    Raises:
        DSLError: If the source does not evaluate, or a strategy in it
            raised — a failure carried in the IR is raised here, because a
            file that half-declared itself is not a file to go on with.
    """
    return _checked(_evaluate(source, name))


def evaluate_file(path: Path | str) -> dict[str, Any]:
    """Evaluate a DSL file and return its IR.

    Args:
        path: The file to read.

    Returns:
        The IR.

    Raises:
        DSLError: If it does not evaluate.
        OSError: If it cannot be read.
    """
    return _checked(_evaluate_file(str(path)))


def _checked(rendered: str) -> dict[str, Any]:
    """Parse the engine's JSON and refuse an IR carrying failures.

    Args:
        rendered: The JSON the engine returned.

    Returns:
        The IR.

    Raises:
        DSLError: If a strategy in the file raised.
    """
    ir: dict[str, Any] = json.loads(rendered)
    failures = ir.get("failures") or []
    if failures:
        told = "\n".join(
            "\n".join([str(one.get("message", "")), *(f"    {line}" for line in one.get("backtrace") or [])])
            for one in failures
        )
        raise DSLError(told)
    return ir
