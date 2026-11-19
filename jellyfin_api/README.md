# jellyfin-api-rust

A Rust client for the Jellyfin API, auto-generated from Jellyfin's OpenAPI
specification.

Currently, this package only exposes models from the Jellyfin API. The tool used
to generate the client doesn't support generating a lot of the API methods.

## Generating the bindings

Dependencies:

- cargo-progenitor fork: `cargo install cargo-progenitor --git https://github.com/coolavery/progenitor.git`
- jq

To generate the bindings, run the generation script: `./generate`
