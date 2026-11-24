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

inputs = {"namumark": "== 제목 ==\n'''안녕'''", "wikitext": "== 제목 ==\n'''안녕'''", "gfm": "## 제목\n\n**안녕**", "html": "<h2>제목</h2><p><strong>안녕</strong></p>"}
markers = {"namumark": ("==", "'''"), "wikitext": ("==", "'''"), "gfm": ("##", "**"), "html": ("<h2", "<b")}
for other in ("wikitext", "gfm", "html"):
    for source, target in (("namumark", other), (other, "namumark")):
        result = wiki.convert({"source": source, "target": target, "input": inputs[source]})
        assert result["ok"] and all(part in result["content"] for part in ("제목", "안녕", *markers[target])), result

calls = []
def unexpected_lookup(*args):
    calls.append(args)
    return None
category = wiki.convert({"source": "namumark", "target": "gfm", "input": "[[분류:Example|Sort]]"}, unexpected_lookup)
assert category["ok"] and category["metadata"]["categories"] == ["Example"] and category["metadata"]["category_sort_keys"] == ["Sort"], category
assert any(d["kind"] == "conversion_loss" for d in category["diagnostics"]), category
strict = wiki.convert({"source": "namumark", "target": "html", "input": "||cell||", "mode": "strict"}, unexpected_lookup)
assert not strict["ok"] and any(d["kind"] == "conversion_loss" for d in strict["diagnostics"]), strict
literal = wiki.convert({"source": "namumark", "target": "html", "input": "{{{literal}}}"}, unexpected_lookup)
assert literal["ok"] and not calls, (literal, calls)
