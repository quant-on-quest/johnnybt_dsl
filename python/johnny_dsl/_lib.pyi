"""What the Rust extension exposes.

Two functions, both returning the IR as JSON text. The typed shell around
them is `johnny_dsl.__init__`, which parses and refuses.
"""

def evaluate(source: str, name: str = ...) -> str: ...
def evaluate_file(path: str) -> str: ...
