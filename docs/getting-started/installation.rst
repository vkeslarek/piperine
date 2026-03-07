============
Installation
============

System Requirements
===================

Piperine requires:

* **Rust 2024 Edition** or later
* **Cargo** (Rust's package manager)
* **Linux, macOS, or Windows** (cross-platform)

Installing Rust
===============

If you don't have Rust installed, get it from `rustup.rs <https://rustup.rs>`_:

.. code-block:: bash

   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

After installation, verify:

.. code-block:: bash

   rustc --version
   cargo --version

Adding Piperine to Your Project
================================

Add Piperine as a dependency in your ``Cargo.toml``:

.. code-block:: toml

   [dependencies]
   piperine = "0.1"

Or use cargo:

.. code-block:: bash

   cargo add piperine

Building from Source
====================

To build Piperine from source:

.. code-block:: bash

   git clone https://github.com/yourusername/piperine.git
   cd piperine
   cargo build --release

Run tests to verify:

.. code-block:: bash

   cargo test

All tests should pass:

.. code-block:: text

   running 18 tests
   test result: ok. 18 passed; 0 failed; 1 ignored

Verification
============

Create a minimal test file ``test.rs``:

.. code-block:: rust

   use piperine::prelude::*;

   fn main() {
       println!("Piperine is ready!");
   }

Run it:

.. code-block:: bash

   cargo run

If you see "Piperine is ready!", you're all set!

Next Steps
==========

* :doc:`first-circuit` - Create your first circuit simulation
* :doc:`concepts` - Understand core Piperine concepts
* :doc:`../tutorials/index` - Follow step-by-step tutorials
