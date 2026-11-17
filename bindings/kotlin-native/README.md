# Kotlin/Native

The binding imports `include/rwikitext.h` through Kotlin/Native cinterop and links the Rust static archive. `convert` accepts the version 1 JSON request and returns the complete JSON response, including content, metadata and diagnostics. See [the native bindings schema](../../docs/native-bindings.md) and `include/rwikitext.h` for the ABI. JSON must not contain literal NUL characters; represent them with JSON escapes.

An optional synchronous `Lookup` receives operation 1 (template text), 2 (page existence), or 3 (file information), and a Unicode title. Return `Reply.Missing`, `Reply.Found(valueJson)`, or `Reply.Error(valueJson)`. Found values are respectively a JSON string, boolean, or object with `url` and optional `width` and `height`. Errors are JSON strings. A thrown exception becomes a lookup failure. Omitting the callback means all lookups are missing.

From the repository root on a stock Kotlin/Native Linux host, install Rust and Kotlin/Native with `cinterop` and `kotlinc-native` on PATH, then run:

```sh
cargo build --release --locked --lib
mkdir -p target/kotlin-native
cinterop -def bindings/kotlin-native/rwikitext.def -compiler-option "-I$PWD/include" -o target/kotlin-native/rwikitext
kotlinc-native bindings/kotlin-native/src/WikiText.kt bindings/kotlin-native/Example.kt \
  -library target/kotlin-native/rwikitext.klib \
  -linker-option "$PWD/target/release/librwikitext.a" \
  -linker-option -ldl -linker-option -lpthread -linker-option -lm -linker-option -lrt -linker-option -lutil \
  -o target/kotlin-native/example
./target/kotlin-native/example.kexe
```

The commands above build for Linux x86-64 on an x86-64 host. To cross-build for Linux ARM64 from a supported x86-64 Linux host, install the Rust `aarch64-unknown-linux-gnu` target and an AArch64 GNU cross-compiler, then run the following. Copy `example.kexe` to a glibc ARM64 Linux system to execute it. Stock Kotlin/Native Linux targets use glibc. See [Kotlin/Native target support](https://kotlinlang.org/docs/native-target-support.html). The example checks successful, missing and failed lookups and non-ASCII text. See [Kotlin cinterop](https://kotlinlang.org/docs/native-c-interop.html) for integrating the generated library into a project.

```sh
rustup target add aarch64-unknown-linux-gnu
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc cargo build --release --locked --lib --target aarch64-unknown-linux-gnu
mkdir -p target/kotlin-native-arm64
cinterop -target linux_arm64 -def bindings/kotlin-native/rwikitext.def -compiler-option "-I$PWD/include" -o target/kotlin-native-arm64/rwikitext
kotlinc-native -target linux_arm64 bindings/kotlin-native/src/WikiText.kt bindings/kotlin-native/Example.kt \
  -library target/kotlin-native-arm64/rwikitext.klib \
  -linker-option "$PWD/target/aarch64-unknown-linux-gnu/release/librwikitext.a" \
  -linker-option -ldl -linker-option -lpthread -linker-option -lm -linker-option -lrt -linker-option -lutil \
  -o target/kotlin-native-arm64/example
```
