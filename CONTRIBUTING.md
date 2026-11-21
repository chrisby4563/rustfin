# Contributing to Rustfin

Bug reports, ideas and pull requests are welcome on the
[issue tracker](https://github.com/chrisby4563/rustfin/issues).

## Development

See [Building](README.md#building). Before sending a pull request:

- `cargo fmt --all`
- `cargo clippy --workspace`
- `MESON_BUILD_ROOT=$PWD/_build CARGO_TARGET_DIR=$PWD/_build/delfin cargo test -p rustfin`

Code, comments and commit messages are in English. User-facing text goes
through the Fluent files in `locales/`; please add new keys to at least
`en-US` (and `de` if you can).

## Code of conduct

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
