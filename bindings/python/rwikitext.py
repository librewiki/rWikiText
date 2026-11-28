"""Synchronous ``ctypes`` binding for the rWikiText version 1 C ABI.

Converts UTF-8 documents between MediaWiki wikitext (``"wikitext"``),
NamuMark (``"namumark"``), GitHub Flavored Markdown (``"gfm"``), and HTML
(``"html"``).

Supported format pairs:
    - ``"wikitext"`` <-> ``"gfm"``, ``"html"``, ``"namumark"``
    - ``"namumark"`` <-> ``"wikitext"``, ``"gfm"``, ``"html"``

Identity conversions and direct ``"gfm"`` <-> ``"html"`` conversions are
unsupported and return a response dict with ``ok=False``.

Example:
    Basic conversion and wiki-data lookup callback::

        from rwikitext import WikiText

        wiki = WikiText("target/release/librwikitext.so")

        def lookup(operation, title):
            if operation == "template_text" and title == "Template:Greeting":
                return "Hello, {{{1}}}!"
            if operation == "page_exists":
                return title == "Main_Page"
            if operation == "file_info" and title == "File:Logo.png":
                return {"url": "https://example.org/logo.png", "width": 200, "height": 100}
            return None

        result = wiki.convert(
            {
                "source": "wikitext",
                "target": "gfm",
                "input": "{{Greeting|{{PAGENAME}}}}\\n[[Category:Docs|Guide]]",
                "context": {"page_title": "Guide"},
                "mode": "best_effort",
            },
            lookup=lookup,
        )
        if result["ok"]:
            print(result["content"])
            print(result["metadata"]["categories"])
"""
import ctypes
import json

_CALLBACK = ctypes.CFUNCTYPE(None, ctypes.c_void_p, ctypes.c_uint32,
                             ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p)
_OPERATIONS = {1: "template_text", 2: "page_exists", 3: "file_info"}


class WikiText:
    """Wrapper around the ``librwikitext`` shared library (C ABI v1).

    An instance may be reused across multiple :meth:`convert` calls, including
    re-entrant calls made from inside a lookup callback.
    """

    def __init__(self, library_path):
        """Load the ``librwikitext`` shared library and verify ABI version 1.

        Args:
            library_path (str | os.PathLike): Path to the compiled
                ``librwikitext`` shared library (for example,
                ``"target/release/librwikitext.so"`` or ``"librwikitext.dylib"``).

        Raises:
            OSError: If the shared library cannot be loaded.
            RuntimeError: If the loaded library does not implement ABI version 1.
        """
        self._lib = ctypes.CDLL(str(library_path))
        self._lib.rwikitext_v1_abi_version.restype = ctypes.c_uint32
        if self._lib.rwikitext_v1_abi_version() != 1:
            raise RuntimeError("unsupported rWikiText ABI version")
        self._lib.rwikitext_v1_reply_set.argtypes = [ctypes.c_void_p, ctypes.c_uint32, ctypes.c_char_p]
        self._lib.rwikitext_v1_reply_set.restype = ctypes.c_int32
        self._lib.rwikitext_v1_convert.argtypes = [ctypes.c_char_p, _CALLBACK, ctypes.c_void_p]
        self._lib.rwikitext_v1_convert.restype = ctypes.c_void_p
        self._lib.rwikitext_v1_free.argtypes = [ctypes.c_void_p]
        self._lib.rwikitext_v1_free.restype = None

    def convert(self, request, lookup=None):
        """Convert a document synchronously and return the decoded response dict.

        Args:
            request (dict): Conversion request mapping with keys:
                - ``source`` (str): Required input format (``"wikitext"``,
                  ``"namumark"``, ``"gfm"``, or ``"html"``).
                - ``target`` (str): Required output format.
                - ``input`` (str): Required UTF-8 document text.
                - ``context`` (dict, optional): Page/site settings with optional
                  ``page_title`` (default ``""``), ``site_language`` (default
                  ``"en"``), and ``reference_time`` (RFC 3339 UTC timestamp,
                  default ``"1970-01-01T00:00:00Z"``).
                - ``mode`` (str, optional): ``"best_effort"`` (default) or
                  ``"strict"`` (fail on conversion loss).
                - ``expansion_limits`` (dict, optional): MediaWiki expansion
                  bounds with optional non-negative integers ``max_depth`` (40),
                  ``max_expansions`` (10000), and ``max_output_bytes`` (2097152).
            lookup (Callable[[str, str], Any] | None, optional): Synchronous
                callback invoked on the calling thread as
                ``lookup(operation, title)``, where ``title`` uses canonical
                namespace casing and underscores (e.g., ``"Template:Greeting"``).
                Return ``None`` to report missing data, or:
                - ``"template_text"`` -> ``str``
                - ``"page_exists"`` -> ``bool`` (``False`` marks page absent)
                - ``"file_info"`` -> ``dict`` with ``"url"`` (``str``) and
                  optional ``"width"`` / ``"height"`` (``int``)
                Any exception raised by ``lookup`` is caught and reported as a
                ``"wiki_lookup"`` failure diagnostic in the returned dict.
                NamuMark input does not invoke ``lookup``.

        Returns:
            dict: Decoded JSON response envelope:
                - On success: ``{"ok": True, "content": str, "metadata": {
                  "categories": list[str],
                  "category_sort_keys": list[str | None],
                  "redirect_target": str | None,
                  "display_title": str | None},
                  "diagnostics": list[dict]}``.
                - On failure: ``{"ok": False, "error": str,
                  "diagnostics": list[dict]}``.
                Each diagnostic dict has ``kind`` (``"conversion_loss"``,
                ``"html_removal"``, ``"wiki_lookup"``, or ``"expansion_limit"``),
                ``start`` and ``end`` (half-open UTF-8 byte offsets), and
                ``message``.

        Raises:
            TypeError: If ``request`` is not JSON-serializable.
            ValueError: If ``request`` contains non-JSON values such as ``NaN``.
            RuntimeError: If the native library returns a null response pointer.

        Example:
            Convert NamuMark to GFM in strict mode::

                res = wiki.convert({
                    "source": "namumark",
                    "target": "gfm",
                    "input": "== Title ==\\n'''Bold'''",
                    "mode": "strict",
                })
                assert res["ok"] is True
                print(res["content"])
        """
        def invoke(_data, operation, title, length, reply):
            try:
                value = None if lookup is None else lookup(_OPERATIONS[operation], ctypes.string_at(title, length).decode("utf-8"))
                status = 0 if value is None else 1
                encoded = None if value is None else json.dumps(value, ensure_ascii=False, allow_nan=False).encode("utf-8")
            except BaseException as exc:
                status = 2
                try:
                    encoded = json.dumps(str(exc), ensure_ascii=True).encode("utf-8")
                except BaseException:
                    encoded = b'"Python lookup failed"'
            self._lib.rwikitext_v1_reply_set(reply, status, encoded)

        callback = _CALLBACK(invoke)
        encoded_request = json.dumps(request, ensure_ascii=False, allow_nan=False).encode("utf-8")
        response = self._lib.rwikitext_v1_convert(encoded_request, callback, None)
        if not response:
            raise RuntimeError("rWikiText native conversion failed")
        try:
            return json.loads(ctypes.string_at(response).decode("utf-8"))
        finally:
            self._lib.rwikitext_v1_free(response)
