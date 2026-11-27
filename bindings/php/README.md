# PHP extension

The PHP 8 extension links the version 1 rWikiText C ABI into a native PHP module. PHP and Rust must target the same architecture and C library. Build from the repository root; the commands below build on native x86-64 or ARM64 Linux, using the distribution's matching PHP development package. Rust's default target must match PHP (`rustc -vV`, `php -i | grep architecture`).

Debian/Ubuntu (glibc, x86-64 or ARM64):

```sh
sudo apt-get update
sudo apt-get install -y build-essential autoconf pkg-config php-cli php-dev cargo rustc
cargo build --locked --release --lib
cd bindings/php
phpize
./configure --with-rwikitext="$(cd ../.. && pwd)"
make -j"$(nproc)"
php -n -d extension="$(pwd)/modules/rwikitext.so" example.php
```

Alpine (musl, x86-64 or ARM64; run the package installation as root):

```sh
apk add build-base autoconf pkgconf php php-dev cargo rust
RUSTFLAGS="-C target-feature=-crt-static" cargo build --locked --release --lib
cd bindings/php
phpize
./configure --with-rwikitext="$(cd ../.. && pwd)"
make -j"$(getconf _NPROCESSORS_ONLN)"
php -n -d extension="$(pwd)/modules/rwikitext.so" example.php
```

If the distribution Rust is older than this repository requires, use a current stable Rust toolchain targeting the host. Use `phpize` and `php-config` from the same PHP installation as the `php` executable; pass `--with-php-config=/path/to/php-config` to `configure` when necessary. See [PHP's phpize instructions](https://www.php.net/manual/en/install.pecl.phpize.php).

`rwikitext_convert(array $request, ?callable $lookup = null): array` accepts `source`, `target` (`wikitext`, `gfm`, `html`, or `namumark`), and `input`. Optional fields are `context` (`page_title`, `site_language`, `reference_time`), `mode` (`best_effort` or `strict`), and `expansion_limits` (`max_depth`, `max_expansions`, `max_output_bytes`). All text must be UTF-8. Supply object-like fields as associative arrays; omit empty optional fields.

The returned associative array contains `ok`, and on success `content`, `metadata`, and `diagnostics`; on failure it contains `error` and `diagnostics`. Diagnostic `start` and `end` are byte offsets into the input. Malformed requests are reported by the ABI as failure results; PHP-side JSON encoding or internal ABI failures throw an exception.

The optional callable receives `(string $operation, string $title)`. Operations are `template_text`, `page_exists`, and `file_info`; return respectively a string, boolean, or associative array with `url` and optional integer `width` and `height`. Return `null` for missing data. A thrown exception becomes a wiki lookup error with its message in the conversion failure. Calls are synchronous on the PHP thread; the callback and values need only live for the duration of `rwikitext_convert`. See `example.php` for lookups, context, metadata, limits, and error handling. The complete request, response, and ABI contract is documented in [Native bindings](../../docs/native-bindings.md).
