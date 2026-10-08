"""Extract comments and actual module/class/function docstrings without executing source."""

import ast
import io
import json
import sys
import tokenize

body = json.loads(sys.stdin.read())
lines = body.splitlines(keepends=True)
starts = [0]
for line in lines:
    starts.append(starts[-1] + len(line))


def offset(line, column, byte_column=False):
    prefix = lines[line - 1][:column]
    if byte_column:
        prefix = lines[line - 1].encode("utf-8")[:column].decode("utf-8")
    absolute = starts[line - 1] + len(prefix)
    return len(body[:absolute].encode("utf-16-le")) // 2


ranges = []
for token in tokenize.generate_tokens(io.StringIO(body).readline):
    if token.type == tokenize.COMMENT:
        ranges.append({"start": offset(*token.start), "end": offset(*token.end), "kind": "line"})

tree = ast.parse(body)
for node in ast.walk(tree):
    if not isinstance(node, (ast.Module, ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)) or not node.body:
        continue
    first = node.body[0]
    if isinstance(first, ast.Expr) and isinstance(first.value, ast.Constant) and isinstance(first.value.value, str):
        ranges.append({"start": offset(first.lineno, first.col_offset, True),
                       "end": offset(first.end_lineno, first.end_col_offset, True), "kind": "docstring"})

sys.stdout.write(json.dumps(sorted(ranges, key=lambda entry: entry["start"])))
