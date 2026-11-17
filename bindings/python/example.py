"""Run with: python bindings/python/example.py path/to/librwikitext.so"""
import sys
from rwikitext import WikiText

wiki = WikiText(sys.argv[1])
seen = []
def lookup(operation, title):
    seen.append((operation, title))
    nested = wiki.convert({"source": "wikitext", "target": "html", "input": "Nested"})
    assert nested["ok"] and "Nested" in nested["content"], nested
    if operation == "template_text":
        return "Hello from Python"
    return None

request = {"source": "wikitext", "target": "html", "input": "{{Greeting}}"}
result = wiki.convert(request, lookup)
assert result["ok"] and "Hello from Python" in result["content"], result
assert any(op == "template_text" for op, _ in seen), seen
assert "metadata" in result and "diagnostics" in result, result

def failure(operation, title):
    raise RuntimeError("Python lookup failed")
failed = wiki.convert(request, failure)
assert not failed["ok"] and "Python lookup failed" in failed["error"], failed
print("Python callback success and failure verified")
