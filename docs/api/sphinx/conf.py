"""Sphinx configuration for rWikiText Python binding API documentation."""

from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "bindings" / "python"))

project = "rWikiText Python API"
author = "LibreWiki"
release = "1.0.1"

extensions = [
    "sphinx.ext.autodoc",
    "sphinx.ext.napoleon",
]

napoleon_google_docstring = True
napoleon_numpy_docstring = False
autodoc_member_order = "bysource"

templates_path = []
exclude_patterns = []

html_theme = "furo"
html_title = "rWikiText Python API"
