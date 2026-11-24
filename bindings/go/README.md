# rWikiText for Go

Convert MediaWiki wikitext to and from GitHub Flavored Markdown (GFM) or HTML in a Go application. The `rwikitext` package uses cgo to call the version 1 C ABI and returns converted content, page metadata, and diagnostics as Go types. Applications can provide wiki data through Go functions, including for offline conversions. [Native bindings guide](../../docs/native-bindings.md)

## Quick start

Use a local rWikiText source checkout on Linux amd64 with glibc. Install Go 1.22 or later, a C compiler and linker targeting that system, and the Rust toolchain and Cargo for `x86_64-unknown-linux-gnu`. Enable cgo and have the C compiler on `PATH`. Set `CC` if you need to select a specific compiler.

Both example workflows were verified on Debian 12 (Linux amd64, glibc 2.36) with Go 1.26.11, GCC 12.2.0, and rustc and Cargo 1.100.0. The Go integration tests also passed with Go 1.22.12.

From the repository root, build the native library and run the example.

```sh
cargo build --locked --release --lib
cd bindings/go
CGO_ENABLED=1 go run ./example
```

The Go package compiles against `include/rwikitext.h` and links `target/release/librwikitext.a` from the checkout. Use Cargo's default target directory. The example prints HTML containing `Hello, Guide!` and the image URL returned by the file lookup, followed by `Categories [Demo]`. [Example source](example/main.go)

After changing the Rust library, rebuild the archive and add `-a` to `go run` or `go build` to force Go to relink it.

## Import from another Go module

Build the archive in the checkout first. Run the following from a separate directory next to `rWikiText`. For an existing Go module, use its directory and skip `go mod init`.

```sh
go mod init example.com/wiki-example
go mod edit -require=github.com/librewiki/rWikiText/bindings/go@v0.0.0
go mod edit -replace=github.com/librewiki/rWikiText/bindings/go=../rWikiText/bindings/go
cp ../rWikiText/bindings/go/example/main.go .
CGO_ENABLED=1 go run .
```

The `replace` directive points Go at the local checkout. Adjust the path to match your directory layout. The application imports `github.com/librewiki/rWikiText/bindings/go` as `rwikitext`.

## Conversion and callbacks

Call `Convert(source, target, input, options)` with `Wikitext` and either `GFM` or `HTML` in one of the two directions. Pass `Options{}` for the default wiki context, best-effort mode, and default expansion limits. Set `Options.Mode` to `Strict` to fail on conversion loss.

Set `Options.Context` to supply the page title, site language, and reference time. The binding sends reference times in UTC at whole-second precision. Set fields in `Options.ExpansionLimits` to bound expansion depth, count, and output bytes. A nil limit uses the native default, and a pointer to zero sets the limit to zero.

Set `Options.Lookups` to provide `TemplateText`, `PageExists`, or `FileInfo` functions. The functions receive canonical titles, such as `Template:Greeting`, synchronously during `Convert`. Each returns a value, a `found` boolean, and an error. An omitted function or `found == false` with a nil error reports missing data. Return `false, true, nil` from `PageExists` to report that a page is known to be absent. A lookup error or panic fails the conversion with a `wiki_lookup` diagnostic.

Inside a lookup, return an error in place of calling `runtime.Goexit` or functions that invoke it, such as `testing.T.Fatal`. The native call needs the Go callback to return.

Successful calls return diagnostics in `Result.Diagnostics`. Use `errors.As` to inspect a `*ConversionError` and its diagnostics on a failed conversion, or a `*NativeError` for request encoding, ABI, and response failures. Diagnostics contain half-open byte ranges in the original UTF-8 input. [Native request, result, and lookup semantics](../../docs/native-bindings.md#conversion-request)

## Testing

After building the archive, run `CGO_ENABLED=1 go test ./...` from `bindings/go`. Add `-a` after rebuilding the archive to force Go to relink it.

## License

rWikiText is licensed under the [MIT License](../../LICENSE).
