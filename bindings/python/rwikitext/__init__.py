"""Synchronous ctypes binding for the rWikiText v1 C ABI."""
import ctypes
import json
from pathlib import Path
import sys

__version__ = "1.0.0"
__all__ = ["WikiText", "__version__"]

_CALLBACK = ctypes.CFUNCTYPE(None, ctypes.c_void_p, ctypes.c_uint32,
                             ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p)
_OPERATIONS = {1: "template_text", 2: "page_exists", 3: "file_info"}


def _default_library_path():
    if sys.platform == "win32":
        filename = "rwikitext.dll"
    elif sys.platform == "darwin":
        filename = "librwikitext.dylib"
    else:
        filename = "librwikitext.so"
    candidate = Path(__file__).resolve().parent / filename
    if not candidate.is_file():
        raise FileNotFoundError(
            f"Bundled native library not found at {candidate}. "
            "Install a platform wheel from PyPI or pass library_path explicitly."
        )
    return candidate


class WikiText:
    def __init__(self, library_path=None):
        resolved_path = _default_library_path() if library_path is None else library_path
        self._lib = ctypes.CDLL(str(resolved_path))
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
        """Return the response dict. lookup(operation, title) returns data or None."""
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
