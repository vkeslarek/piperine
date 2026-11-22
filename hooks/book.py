"""MkDocs hook that publishes the MSL Book on the documentation site.

The Book's sources live in `docs/book/` (`content.html`, `template.html`,
`build.py`) and stay out of the page tree (`exclude_docs`). Before each build
this hook runs `build.py`; after the build it writes the generated Book, as a
complete HTML document, to `<site>/book/index.html`, which the navigation links
to as "The MSL Book".
"""
import pathlib
import runpy

BOOK_SOURCES = pathlib.Path(__file__).resolve().parent.parent / "docs" / "book"
DOCUMENT_HEAD = (
    '<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n'
    '<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">\n'
    "<style>body{margin:0}img{max-width:100%}[hidden]{display:none!important}</style>\n"
    "</head>\n<body>\n"
)
DOCUMENT_TAIL = "\n</body>\n</html>\n"


def on_pre_build(config, **kwargs):
    """Regenerates docs/book/index.html from the Book's sources."""
    runpy.run_path(str(BOOK_SOURCES / "build.py"), run_name="__main__")


def on_post_build(config, **kwargs):
    """Writes the Book into the built site as book/index.html."""
    body = (BOOK_SOURCES / "index.html").read_text(encoding="utf-8")
    target = pathlib.Path(config["site_dir"]) / "book" / "index.html"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(DOCUMENT_HEAD + body + DOCUMENT_TAIL, encoding="utf-8")
