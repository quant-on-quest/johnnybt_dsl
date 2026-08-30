"""What the Rust extension exposes.

The typed shell around it is `johnny_dsl.__init__`.
"""

from collections.abc import Sequence
from typing import Any

def run(chunks: Sequence[Any], answer: str = ...) -> str: ...
def compile(source: str, name: str = ...) -> bytes: ...
