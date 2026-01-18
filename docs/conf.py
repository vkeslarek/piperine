project = 'Piperine'
html_title = "Piperine"
copyright = '2026, Keslarek'
author = 'Vinicius Keslarek'
release = '0.0.1-beta'

extensions = ["sphinxcontrib_rust", "myst_parser"]
source_suffix = {
    ".rst": "restructuredtext",
}

myst_enable_extensions = {
    'sphinx_tags',
}
rust_crates = {
    "piperine": "..",
}
html_permalinks_icon = '<span>#</span>'
html_theme = 'sphinxawesome_theme'
rust_doc_dir = "docs/crates/"