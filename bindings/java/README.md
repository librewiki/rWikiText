# Java (JNI)

The Java binding loads the native C ABI through JNI. `WikiText.convert` accepts a UTF-8 JSON request and returns the complete JSON response, including content, metadata and diagnostics. See [the native bindings schema](../../docs/native-bindings.md) and `include/rwikitext.h` for the ABI. JSON must not contain literal NUL characters; represent them with JSON escapes.

The optional synchronous `Lookup` receives operation 1 (template text), 2 (page existence), or 3 (file information), and a Unicode title. Return `Reply.missing()`, `Reply.found(valueJson)`, or `Reply.error(valueJson)`. Found values are respectively a JSON string, boolean, or object with `url` and optional `width` and `height`. Errors are JSON strings. A thrown exception or null reply becomes a lookup failure. Returning null as the callback means all lookups are missing.

From the repository root on Linux, install Rust, a C compiler and a JDK (including JNI headers), then run:

```sh
cargo build --release --locked --lib
export JAVA_HOME="$(dirname "$(dirname "$(readlink -f "$(command -v javac)")")")"
mkdir -p target/java
cc -shared -fPIC -Iinclude -I"$JAVA_HOME/include" -I"$JAVA_HOME/include/linux" \
  bindings/java/native/rwikitext_jni.c -Ltarget/release -lrwikitext \
  -Wl,-rpath,'$ORIGIN' -o target/java/librwikitext_jni.so
cp target/release/librwikitext.so target/java/
javac -encoding UTF-8 -d target/java bindings/java/src/org/librewiki/rwikitext/WikiText.java bindings/java/Example.java
java -Djava.library.path=target/java -cp target/java Example
```

Build and run natively with matching Rust, JDK and C compiler architectures. This procedure targets Linux glibc and musl on x86-64 and ARM64. On a musl host with a musl-compatible JDK and Rust host toolchain, replace the `cargo build` line with the following, then run the remaining commands unchanged: The example checks successful, missing and failed lookups and non-ASCII text.


```sh
RUSTFLAGS="-C target-feature=-crt-static" cargo build --release --locked --lib
```
