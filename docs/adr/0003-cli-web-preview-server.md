# Serve the CLI web preview from a zero-dependency local HTTP server

The `rwikitext serve` subcommand exposes interactive document conversion and visual preview in a local web UI (`--bind`, `--port`). It runs directly inside the CLI binary using `std::net::TcpListener` with an embedded HTML/JS page instead of adding an HTTP framework or async runtime to `Cargo.toml`.

## Separation of conversion and lazy preview

The server splits conversion (`POST /api/convert`) from preview rendering (`POST /api/preview`). Measuring conversion time in microseconds covers only `convert_with_options`, without mixing in secondary rendering cost. The web UI requests preview HTML lazily only when the user opens or edits inside the Preview tab, and renders the fragment inside a sandboxed `<iframe>` (`sandbox=""`) so converted markup cannot execute scripts or alter host styles.

## Conversion-contract boundary

For non-HTML targets, `POST /api/preview` renders `wikitext` and `namumark` through `rwikitext` to HTML in best-effort mode, and renders `gfm` through `comrak` with unsafe HTML disabled. This preview path exists solely for visual inspection in the CLI web UI and does not add direct GFM ↔ HTML to the library conversion contract defined in [ADR 0001](0001-parsoid-and-document-fidelity.md).

## Consequences

- The library crate and C ABI (`src/lib.rs`, `src/ffi.rs`) remain unaffected by web server code or extra dependencies.
- The server targets local interactive use (one OS thread per connection, bounded header/body sizes, and socket timeouts) rather than internet-facing production hosting.
