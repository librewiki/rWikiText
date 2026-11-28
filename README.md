# rWikiText

[한국어](#rwikitext) · [English](#rwikitext-english)

rWikiText는 MediaWiki 위키텍스트, 나무마크(NamuMark), GitHub Flavored Markdown(GFM), HTML 사이에서 문서를 변환하는 Rust 라이브러리이자 명령줄 도구다.

위키 문서를 Markdown이나 HTML 작업 흐름으로 옮기거나 외부 문서를 위키로 들여올 때, 또는 애플리케이션 안에 변환 기능을 직접 넣을 때 쓴다. 변환을 실행하면 결과 본문과 구조화된 페이지 메타데이터, 그리고 원본 입력의 바이트 범위로 변환 손실 위치를 짚어 주는 진단 정보를 돌려준다.

나무마크 지원은 **알파 단계**다. 첫 릴리스에서는 변환 범위를 의도적으로 좁게 잡았고 지원하지 않는 문법은 위치 정보와 함께 진단으로 알린다. 문법 기준 문서와 수작업으로 검수한 독립 픽스처는 고정해 두었지만 실제 나무위키 렌더러와의 호환성은 아직 검증하지 않았다. 자세한 범위와 제외 항목은 [나무마크 계약 및 제외 범위](docs/namumark.md)와 [후보 검증 현황](docs/compatibility.md#namumark-candidate-verification)에 정리되어 있다.

## 설치

- **Rust 라이브러리 ([crates.io](https://crates.io/crates/rwikitext) · [docs.rs](https://docs.rs/rwikitext))**: `cargo add rwikitext` 또는 `Cargo.toml`의 `[dependencies]`에 `rwikitext = "1.0"` 추가
- **CLI 바이너리**: `cargo install rwikitext`
- **Node.js ([npm](https://www.npmjs.com/package/@librewiki/rwikitext))**: `npm install @librewiki/rwikitext`
- **기타 언어 바인딩 및 프리빌드 아카이브**: [GitHub Releases](https://github.com/librewiki/rWikiText/releases/tag/v1.0.0)

## 빠른 시작

소스 체크아웃에서 빌드하려면 최신 Rust 툴체인, Cargo, 대상 플랫폼용 링커가 필요하다. Rust 라이브러리와 CLI는 호출자가 넘긴 위키 데이터를 사용해 로컬에서 동작한다. 각 네이티브 바인딩에 필요한 추가 도구는 [네이티브 바인딩 요구사항](docs/native-bindings.md)에서 확인할 수 있다.

저장소 루트에서 기본 제공 위키텍스트 픽스처를 HTML로 변환해 볼 수 있다.

```sh
cargo run --locked -- wikitext html tests/fixtures/paragraphs.wiki
```

표준 출력에 `<p>` 요소 두 개가 나온다. 두 번째 문단에는 `Second paragraph &amp; more.`가 들어간다.

브라우저에서 바로 변환 결과를 보고 싶다면 로컬 웹 미리보기 서버를 띄우면 된다.

```sh
cargo run --locked -- serve --bind 127.0.0.1 --port 8080
```

실행 후 출력되는 주소(`http://127.0.0.1:8080`)를 브라우저로 열면 상단에서 원본·대상 마크업을 고를 수 있다. 왼쪽에 원본을 입력하면 오른쪽에서 Raw 결과나 지연 로딩(lazy loading)되는 Preview를 바로 볼 수 있고 서버 측 변환 소요 시간은 마이크로초(`µs`) 단위로 표시된다.

## 변환 범위

지원하는 형식 쌍은 MediaWiki 위키텍스트 ↔ GFM, MediaWiki 위키텍스트 ↔ HTML, 나무마크 ↔ 나머지 세 형식(MediaWiki 위키텍스트, GFM, HTML)이다. 같은 형식으로 바꾸는 항등 변환과 GFM ↔ HTML 직접 변환은 지원하지 않는다. 아래 내용은 MediaWiki 변환 기준이다.

- 위키텍스트와 GFM 사이, 위키텍스트와 HTML 사이를 양방향으로 변환한다. 각 변환은 입력이나 출력 중 한쪽이 반드시 위키텍스트다.
- 문단, 제목, 링크, 강조, 서식 유지 텍스트, 가로선, 목록, 표 같은 핵심 문서 구조를 변환한다. GFM 출력에서는 가능하면 네이티브 Markdown 표를 쓰고 병합 셀처럼 Markdown으로 표현하기 어려운 구조에는 HTML을 사용한다.
- 명시적인 위키 컨텍스트와 호출자 제공 데이터로 틀(template)과 지원하는 핵심 파서 함수를 확장한다. HTML 입력에 Parsoid 메타데이터가 들어 있으면 검증을 거쳐 틀 호출과 인자를 복원한다.
- 분류와 정렬 키, 넘겨주기(redirect), 표시 제목(display title)을 페이지 메타데이터로 돌려준다. 위키텍스트와 HTML은 변환된 문서 안에 이 정보를 담을 수 있다. GFM은 메타데이터를 별도로 반환하고 본문만 저장할 때 사라지는 정보를 진단으로 알린다.
- 내장 HTML과 외부에서 들여온 HTML에는 MediaWiki의 허용 요소·속성·URL 정책을 적용한다. 최선 변환(best-effort) 모드는 손실과 제거 내역을 진단으로 보고한다. 엄격(`strict`) 모드는 변환 손실이 생기면 실패한다.

호환성은 MediaWiki 1.46.2와 Parsoid HTML 2.8.0을 기준으로 삼는다. 왕복(round-trip) 충실도는 렌더링된 내용, 문서 구조, 페이지 메타데이터를 보존하는 데 초점을 맞추며 의미가 같은 위키텍스트라도 표기나 공백은 달라질 수 있다. 첫 릴리스 범위에는 MediaWiki 확장 태그와 확장 파서 함수가 포함되지 않는다. 검증된 사례와 남은 차이는 [호환성 기준선](docs/compatibility.md)과 [파서 케이스 목록](docs/parser-case-inventory.md)에서 볼 수 있다.

나무마크 프로필은 일반 텍스트, 제목, 굵게·기울임, 링크, 이스케이프, 리터럴 코드, 최상위 목록, 줄바꿈, 가로선, 분류, 넘겨주기를 다룬다. 표, 각주, 미디어, 문서 포함(include)과 기타 매크로는 변환 손실로 진단한다. 나무마크 입력은 자체 URL·문서 제목 규칙을 따르며 위키 데이터 콜백을 호출하지 않는다. 나무마크 기준 픽스처는 MediaWiki/Parsoid 호환성 자료와 분리해 관리한다. [나무마크 상세 문서](docs/namumark.md)

## 사용법

### Rust 라이브러리

[crates.io](https://crates.io/crates/rwikitext)에서 `rwikitext`를 의존성으로 추가하거나([docs.rs/rwikitext](https://docs.rs/rwikitext) API 문서) 로컬 소스 체크아웃을 경로 의존성으로 연결할 수 있다. 아래 예제는 `crates.io` 버전 또는 애플리케이션 디렉터리 옆의 로컬 체크아웃을 사용하는 방법이다.

```toml
[dependencies]
rwikitext = "1.0"
# 또는 로컬 체크아웃 사용 시:
# rwikitext = { path = "../rWikiText" }
```

CLI만 바로 설치하려면 `cargo install rwikitext`를 실행한다.

애플리케이션의 `src/main.rs`에 아래 코드를 넣고 프로젝트 루트에서 `cargo run`을 실행하면 `<p>Hello <b>wiki</b></p>`가 출력된다.

```rust
use rwikitext::{convert, ConversionRequest, Format, WikiContext};

fn main() -> Result<(), rwikitext::ConversionError> {
    let context = WikiContext::default();
    let result = convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input: "Hello '''wiki'''",
        context: &context,
    })?;
    println!("{}", result.content);
    Ok(())
}
```

나무마크를 지정하려면 `Format::NamuMark`를 쓴다. 변환 손실을 오류로 막으려면 `convert_with_mode`에 `ConversionMode::Strict`를 넘긴다. `WikiData` 구현을 주입하거나 `ExpansionLimits`를 조정할 때는 `convert_with_options`를 사용한다. MediaWiki 입력을 처리할 때 라이브러리는 호출 스레드에서 동기적으로 콜백을 불러 틀 본문, 문서 존재 여부, 파일 정보를 조회한다. 콜백은 데이터를 반환하거나 없음으로 알릴 수 있고 위치가 담긴 진단과 함께 변환을 실패시킬 수도 있다. [Rust API](src/lib.rs) · [콜백과 확장 한도](src/wiki_data.rs)

### 명령줄 도구 (CLI)

원본 형식과 대상 형식 자리에 `wikitext`, `namumark`, `gfm`, `html` 중 지원하는 쌍을 넘긴다. 파일 경로를 생략하거나 `-`를 쓰면 표준 입력을 읽는다. CLI는 변환된 본문을 표준 출력으로, 진단 메시지를 표준 오류로 내보낸다. `--strict`를 붙이면 변환 손실이 있을 때 종료 코드 1로 끝낸다.

`--wiki-data`를 주면 JSON 파일에서 오프라인 위키 조회 데이터를 불러온다. 선택 항목인 `context`, `templates`, `pages`, `files` 객체로 위키 컨텍스트, 틀 본문, 문서 존재 여부 불리언, URL과 선택적 픽셀 크기를 담은 파일 레코드를 제공할 수 있다. 항목을 비워 두거나 `null`을 넣으면 데이터 없음으로 처리된다. `error` 문자열이 든 객체를 넣으면 조회 실패를 재현한다. [위키 데이터 예제](tests/fixtures/wiki-data.json)

`--page-title`, `--site-language`, `--reference-time` 옵션은 JSON의 해당 컨텍스트 필드를 덮어쓴다. 기본값은 빈 문서 제목, `en`, `1970-01-01T00:00:00Z`다.

저장소 루트에서 예제 위키 데이터로 중첩 틀을 확장해 볼 수 있다. 출력 HTML은 `example`을 굵게 렌더링하고 `data-mw` 틀 포함 메타데이터를 함께 싣는다.

```sh
echo "{{Outer|example}}" | cargo run --locked -- wikitext html - --wiki-data tests/fixtures/wiki-data.json
```

#### 로컬 웹 미리보기 서버 (`serve`)

대화형으로 변환 결과와 미리보기를 확인하려면 `serve` 서브커맨드를 실행한다.

```sh
cargo run --locked -- serve [--port PORT] [--bind ADDR]
```

| 옵션 | 기본값 | 설명 |
| --- | --- | --- |
| `--bind ADDR` | `127.0.0.1` | HTTP 서버가 바인딩할 주소 (`--bind=ADDR` 형식도 지원) |
| `--port PORT` | `8080` | 수신 포트 (`0`을 주면 OS가 빈 포트를 자동 할당, `--port=PORT` 형식도 지원) |
| `--strict` | 비활성 | 웹 UI의 기본 변환 모드를 엄격 모드로 설정 |
| `--wiki-data JSON` | 없음 | 오프라인 위키 조회 데이터 JSON 경로 |
| `--page-title`, `--site-language`, `--reference-time` | 기본 위키 컨텍스트 | 서버 요청에 적용할 기본 위키 컨텍스트 |

웹 페이지 상단에서 원본·대상 마크업과 `Strict` 모드를 선택할 수 있으며 서버 측 변환 시간(`µs`)이 함께 표시된다. 왼쪽 입력란에 원본을 적으면 오른쪽 영역에 변환된 Raw 출력이 바로 나타난다. `Preview` 탭을 열 때만 `/api/preview`로 미리보기 HTML을 지연 로딩해 샌드박스 `<iframe>` 안에 렌더링한다.

### 네이티브 바인딩

C, Python, PHP, Java, Kotlin/Native, Go는 버전 1 C ABI(`librwikitext`)로 같은 변환 코어를 호출한다. Node.js 바인딩([`@librewiki/rwikitext`](https://www.npmjs.com/package/@librewiki/rwikitext))은 `napi-rs`로 Rust 코어를 `.node` 바이너리에 직접 컴파일해 배포한다. 별도 공유 라이브러리 없이 `npm install @librewiki/rwikitext`로 설치할 수 있으며 Linux(`x64`/`arm64`, glibc·musl), macOS(`x64`/`arm64`), Windows(`x64`/`arm64`)용 프리빌드 바이너리와 TypeScript 타입 정의, `convertAsync`를 함께 제공한다.

첫 릴리스 검증 매트릭스는 Linux x86-64와 ARM64의 glibc 및 musl 환경을 대상으로 하며 Kotlin/Native는 glibc만 대상이다. `v1.0.0` 릴리스에서 Linux glibc(14/14)와 musl(12/12) 런타임 검증을 마쳤고 언어·플랫폼별 바이너리 묶음은 [GitHub Releases](https://github.com/librewiki/rWikiText/releases/tag/v1.0.0)에서 받을 수 있다. [네이티브 바인딩 빌드 및 콜백 예제](docs/native-bindings.md) · [대상 매트릭스](docs/adr/0002-versioned-c-abi-and-wiki-data-callbacks.md) · [Node.js 바인딩](bindings/node/README.md) · [Go 바인딩](bindings/go/README.md) · [glibc 검증](https://github.com/librewiki/rWikiText/issues/26) · [musl 검증](https://github.com/librewiki/rWikiText/issues/27) · [첫 공개 릴리스](https://github.com/librewiki/rWikiText/issues/28)

## 개발 및 테스트

저장소 루트에서 아래 명령을 실행한다.

```sh
cargo build --locked
cargo test --locked
```

기본 테스트 스위트는 Rust API와 CLI를 통해 변환, 왕복 변환, 진단, 위키 데이터 콜백, 로컬 `serve` 웹 서버 엔드포인트를 검증한다. 전체 호환성 테스트를 돌리려면 검증된 MediaWiki 아카이브와 로컬 PHP 런타임이 추가로 필요하다. [전체 호환성 테스트 안내](docs/compatibility.md#first-release-verification)

`develop`·`main` 대상 풀 리퀘스트에서는 [GitHub Actions CI](.github/workflows/ci.yml)가 전체 통합 테스트(PHP 8.4와 고정 MediaWiki 1.46.2 아카이브 포함)와 네이티브 바인딩 테스트(C, Python, Go, Java JNI, Node.js `napi-rs`, PHP 확장)를 실행한다. `release/*` 브랜치가 `main`에 병합되거나 수동 실행(`workflow_dispatch`)되면 [릴리스 워크플로](.github/workflows/release.yml)가 OIDC Trusted Publishing으로 `crates.io`(`rwikitext`)와 `npm`(`@librewiki/rwikitext` 및 플랫폼별 패키지 8종)에 패키지를 배포한다.

CLI([`src/main.rs`](src/main.rs), 로컬 웹 서버 [`src/serve.rs`](src/serve.rs))와 C ABI([`src/ffi.rs`](src/ffi.rs))는 [`src/lib.rs`](src/lib.rs)의 공개 Rust 파사드를 호출한다. 이 파사드가 [`src/pipeline.rs`](src/pipeline.rs)의 내부 변환 파이프라인에 작업을 넘긴다. 형식별 파싱은 [`src/wikitext.rs`](src/wikitext.rs), [`src/namumark/`](src/namumark/mod.rs), [`src/html.rs`](src/html.rs)가 맡고 인라인 렌더링은 [`src/render.rs`](src/render.rs), 원본 입력 바이트 범위 매핑은 [`src/spans.rs`](src/spans.rs)가 담당한다. MediaWiki 위키텍스트 입력에서는 파이프라인이 호출자의 위키 데이터를 써서 틀을 확장하고 링크를 해석한 뒤 변환 본문과 페이지 메타데이터·진단을 돌려준다. 자세한 용어와 설계 결정은 [도메인 용어집](GLOSSARY.md), [변환 설계 ADR](docs/adr/0001-parsoid-and-document-fidelity.md), [로컬 웹 미리보기 서버 ADR](docs/adr/0003-cli-web-preview-server.md)을 참고한다.

기여할 때는 저장소의 이슈, 브랜치, 풀 리퀘스트 절차를 따른다. [기여자 및 에이전트 지침](AGENTS.md) · [작업 수명주기](docs/agents/issue-tracker.md#work-lifecycle)

## 라이선스

rWikiText의 코드는 [MIT 라이선스](LICENSE)를 따른다. 함께 포함된 나무위키 도움말 스냅샷은 CC BY-NC-SA 2.0 KR 라이선스가 별도로 적용된다. 자세한 내용은 [서드파티 고지](docs/THIRD_PARTY_NOTICES.md)를 확인한다.

---

## rWikiText (English)

rWikiText is a Rust library and command-line tool for converting documents between MediaWiki wikitext, NamuMark, GitHub Flavored Markdown (GFM), and HTML.

Use it to move wiki documents into Markdown or HTML workflows, import documents into a wiki, or embed conversion in an application. The library returns converted content, structured page metadata, and diagnostics that identify conversion losses by byte range in the original input.

NamuMark (나무마크) support is currently **alpha**. It has a deliberately small first-release conversion profile with located diagnostics for unsupported syntax. Its syntax reference and independent hand-reviewed fixtures are pinned; live NamuWiki renderer compatibility remains unverified. See the [NamuMark contract and exclusions](docs/namumark.md) and [candidate verification](docs/compatibility.md#namumark-candidate-verification).

### Installation

- **Rust library ([crates.io](https://crates.io/crates/rwikitext) · [docs.rs](https://docs.rs/rwikitext))**: `cargo add rwikitext` or add `rwikitext = "1.0"` under `[dependencies]` in `Cargo.toml`
- **CLI binary**: `cargo install rwikitext`
- **Node.js ([npm](https://www.npmjs.com/package/@librewiki/rwikitext))**: `npm install @librewiki/rwikitext`
- **Other native bindings & prebuilt archives**: [GitHub Releases](https://github.com/librewiki/rWikiText/releases/tag/v1.0.0)

### Quick start

Build from a source checkout with a current Rust toolchain, Cargo, and the linker for your Rust target. The Rust library and CLI run locally with caller-supplied wiki data. Check the native binding guide for the additional tools each binding requires. [Native binding requirements](docs/native-bindings.md)

From the repository root, convert the supplied wikitext fixture to HTML.

```sh
cargo run --locked -- wikitext html tests/fixtures/paragraphs.wiki
```

Standard output contains two `<p>` elements. The second contains `Second paragraph &amp; more.`

To inspect conversions interactively in a browser, start the local web preview server:

```sh
cargo run --locked -- serve --bind 127.0.0.1 --port 8080
```

Open the printed address (`http://127.0.0.1:8080`) to select source and target markups at the top, edit source text on the left, view raw output or lazily loaded preview on the right, and inspect server-side conversion time in microseconds (`µs`).

### Conversion scope

The format pairs are MediaWiki wikitext ↔ GFM, MediaWiki wikitext ↔ HTML, and NamuMark ↔ each of the other three formats. Identity conversions and direct GFM ↔ HTML are unsupported. The following describes the MediaWiki conversions.

- Convert in both directions between wikitext and GFM or between wikitext and HTML. Each conversion takes wikitext as either its input or its output.
- Convert core document structure, including paragraphs, headings, links, emphasis, preformatted text, horizontal rules, lists, and tables. GFM uses native Markdown tables where possible and HTML for structures such as merged cells.
- Expand templates and supported core parser functions with explicit wiki context and caller-supplied data. HTML can carry Parsoid metadata that the importer validates to recover template calls and arguments.
- Return categories and their sort keys, redirects, and display titles as page metadata. Wikitext and HTML can carry these in the converted document. For GFM, the metadata is returned separately and diagnostics identify what saving the body alone would lose.
- Apply MediaWiki rules for elements, attributes, and URLs to embedded and imported HTML. Best-effort conversion reports losses and removals. Strict mode fails on conversion loss.

Compatibility is based on MediaWiki 1.46.2 and Parsoid HTML 2.8.0. Round-trip fidelity concerns rendered content, document structure, and page metadata. Equivalent wikitext can differ in spelling and whitespace. The first-release scope excludes MediaWiki extension tags and parser functions. See the verified cases and remaining differences in the [compatibility baseline](docs/compatibility.md) and [parser case inventory](docs/parser-case-inventory.md).

The NamuMark profile covers text, headings, bold/italic, links, escaping, literal code, top-level lists, line breaks, rules, categories, and redirects. Tables, footnotes, media, includes, and other macros are diagnosed as conversion losses. NamuMark input uses its own URL and page-title rules and makes no wiki data callbacks. Its reference fixtures are separate from the MediaWiki/Parsoid compatibility evidence. [NamuMark details](docs/namumark.md)

### Usage

#### Rust library

Add `rwikitext` from [crates.io](https://crates.io/crates/rwikitext) (API docs: [docs.rs/rwikitext](https://docs.rs/rwikitext)), or point to a local source checkout under `[dependencies]` in your application's `Cargo.toml`.

```toml
[dependencies]
rwikitext = "1.0"
# Or from a local checkout next to your application directory:
# rwikitext = { path = "../rWikiText" }
```

To install only the CLI binary, run `cargo install rwikitext`.

Put this in your application's `src/main.rs` and run `cargo run` from its root. It prints `<p>Hello <b>wiki</b></p>`.

```rust
use rwikitext::{convert, ConversionRequest, Format, WikiContext};

fn main() -> Result<(), rwikitext::ConversionError> {
    let context = WikiContext::default();
    let result = convert(ConversionRequest {
        source: Format::Wikitext,
        target: Format::Html,
        input: "Hello '''wiki'''",
        context: &context,
    })?;
    println!("{}", result.content);
    Ok(())
}
```

Use `Format::NamuMark` to select NamuMark. Use `convert_with_mode` with `ConversionMode::Strict` to reject conversion loss. Use `convert_with_options` to supply a `WikiData` implementation and adjust `ExpansionLimits`. For MediaWiki input, the library calls back synchronously on the calling thread for template text, page existence, and file information. A lookup can return data, report missing data, or fail the conversion with a located diagnostic. [Rust API](src/lib.rs) · [Callbacks and expansion limits](src/wiki_data.rs)

#### Command-line tool

Pass the source and target formats as `wikitext`, `namumark`, `gfm`, or `html`, using a supported pair above. Omit the file path or use `-` to read standard input. The CLI writes converted content to standard output and diagnostics to standard error. Add `--strict` to exit with status 1 on conversion loss.

Use `--wiki-data` to load offline wiki lookups from a JSON file. The optional `context`, `templates`, `pages`, and `files` objects supply wiki context, template text, page-existence booleans, and file records with a URL and optional dimensions in pixels. Omit a lookup title or use `null` for missing data. Use an object with an `error` string to simulate a lookup failure. [Example wiki data](tests/fixtures/wiki-data.json)

`--page-title`, `--site-language`, and `--reference-time` override the corresponding JSON context fields. The defaults are an empty page title, `en`, and `1970-01-01T00:00:00Z`.

From the repository root, expand a nested template using the example wiki data. The HTML renders `example` in bold and includes `data-mw` transclusion metadata.

```sh
echo "{{Outer|example}}" | cargo run --locked -- wikitext html - --wiki-data tests/fixtures/wiki-data.json
```

##### Local web preview server (`serve`)

Run the `serve` subcommand to launch an interactive local web UI:

```sh
cargo run --locked -- serve [--port PORT] [--bind ADDR]
```

| Option | Default | Description |
| --- | --- | --- |
| `--bind ADDR` | `127.0.0.1` | Bind address for the HTTP server (`--bind=ADDR` also supported) |
| `--port PORT` | `8080` | Listening port (`0` assigns an ephemeral port; `--port=PORT` also supported) |
| `--strict` | Disabled | Default conversion mode in the web UI |
| `--wiki-data JSON` | None | Path to offline wiki lookup JSON |
| `--page-title`, `--site-language`, `--reference-time` | Default wiki context | Default wiki context applied to server requests |

The web UI provides source and target format selectors, a `Strict` toggle, and server-side conversion timing in microseconds (`µs`) at the top. Editing the source text on the left updates the raw output on the right; preview HTML is requested lazily via `/api/preview` only when the `Preview` tab is active and rendered inside a sandboxed `<iframe>`.

#### Native bindings

C, Python, PHP, Java, Kotlin/Native, and Go call the shared conversion core through the version 1 C ABI (`librwikitext`). The Node.js binding ([`@librewiki/rwikitext`](https://www.npmjs.com/package/@librewiki/rwikitext)) uses `napi-rs` to compile the Rust converter directly into `.node` binaries, installed via `npm install @librewiki/rwikitext` with no separate shared library. Prebuilt packages cover Linux (`x64`/`arm64`, glibc and musl), macOS (`x64`/`arm64`), and Windows (`x64`/`arm64`), with TypeScript definitions and `convertAsync`.

The first-release verification matrix targets Linux x86-64 and ARM64 with glibc or musl (Kotlin/Native targets glibc in that matrix). `v1.0.0` passed both Linux glibc (14/14) and musl (12/12) runtime verification matrices; prebuilt per-language and per-platform archives are published on [GitHub Releases](https://github.com/librewiki/rWikiText/releases/tag/v1.0.0). [Native binding build and callback examples](docs/native-bindings.md) · [Target matrix](docs/adr/0002-versioned-c-abi-and-wiki-data-callbacks.md) · [Node.js binding](bindings/node/README.md) · [Go binding](bindings/go/README.md) · [glibc verification](https://github.com/librewiki/rWikiText/issues/26) · [musl verification](https://github.com/librewiki/rWikiText/issues/27) · [First public release](https://github.com/librewiki/rWikiText/issues/28)

### Development and testing

Run these commands from the repository root.

```sh
cargo build --locked
cargo test --locked
```

The default suite exercises conversion, round trips, diagnostics, wiki data callbacks, and the local `serve` web server endpoints through the Rust API and CLI. The full compatibility suite also uses a verified MediaWiki archive and a local PHP runtime. [Full compatibility test instructions](docs/compatibility.md#first-release-verification)

Pull requests targeting `develop` or `main` run [GitHub Actions CI](.github/workflows/ci.yml), covering the full integration suite (including PHP 8.4 and the pinned MediaWiki 1.46.2 archive) and native binding tests (C, Python, Go, Java JNI, Node.js `napi-rs`, and the PHP extension). Merging a `release/*` PR into `main` (or triggering `workflow_dispatch`) runs the [release workflow](.github/workflows/release.yml), which publishes `rwikitext` to `crates.io` and `@librewiki/rwikitext` (plus 8 platform packages) to `npm` via OIDC Trusted Publishing.

The CLI ([`src/main.rs`](src/main.rs), local web server [`src/serve.rs`](src/serve.rs)) and C ABI ([`src/ffi.rs`](src/ffi.rs)) call the public Rust facade in [`src/lib.rs`](src/lib.rs), which delegates to the internal conversion pipeline in [`src/pipeline.rs`](src/pipeline.rs). Dedicated internal modules handle format parsing (including [`src/wikitext.rs`](src/wikitext.rs), [`src/namumark/`](src/namumark/mod.rs), and [`src/html.rs`](src/html.rs)), inline rendering ([`src/render.rs`](src/render.rs)), and original-input span mapping ([`src/spans.rs`](src/spans.rs)). For MediaWiki wikitext input the pipeline also expands templates and resolves links using the caller's wiki data, returning converted content with page metadata and diagnostics. See the [domain glossary](GLOSSARY.md), [conversion design](docs/adr/0001-parsoid-and-document-fidelity.md), and [local web preview server ADR](docs/adr/0003-cli-web-preview-server.md).

For contributions, follow the repository's issue, branch, and pull request workflow. [Contributor and agent instructions](AGENTS.md) · [Work lifecycle](docs/agents/issue-tracker.md#work-lifecycle)

### License

rWikiText's code is licensed under the [MIT License](LICENSE). The supplied NamuWiki help snapshot is separately licensed under CC BY-NC-SA 2.0 KR; see [third-party notices](docs/THIRD_PARTY_NOTICES.md).
