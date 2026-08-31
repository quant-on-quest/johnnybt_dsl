"""What the Rust extension exposes.

The typed shell around it is `johnnybt_dsl.__init__`.
"""

from collections.abc import Sequence
from typing import Any

def run(chunks: Sequence[Any], answer: str = ..., context: str | None = ...) -> str: ...
def compile(source: str, name: str = ...) -> bytes: ...
