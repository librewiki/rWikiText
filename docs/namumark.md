# NamuMark conversion contract

Baseline v1 was fixed before implementation for [#47](https://github.com/librewiki/rWikiText/issues/47). It adds six directions: NamuMark ↔ MediaWiki wikitext, NamuMark ↔ GFM, NamuMark ↔ HTML. Existing four directions remain supported; identity and direct GFM ↔ HTML conversions remain unsupported. Preserve existing identifiers and ABI values.

## Reference and independent expectations

The user-supplied [syntax help snapshot](namumark_manual_r927_20261124.txt) is pinned by SHA-256 `520eedc1f02128f0e4eb35c044bdd466ad7e9fc51458b565942ca0b681f9b608`, verified locally on 2026-11-24 with `Get-FileHash docs/namumark_manual_r927_20261124.txt -Algorithm SHA256`. The r927 revision/date are user supplied, not independently verified online. Earlier direct source retrieval failed. No executable NamuMark renderer is pinned or claimed to have run. The linked advanced/developer help is outside this snapshot.

The [fixture manifest](namumark-baseline.json) fixes original hand-reviewed semantic expectations independently of converter output, before implementation. It is a snapshot-derived oracle, not external renderer evidence. Do not generate expectations from rWikiText or rely only on self-round-trips. The help is separately CC BY-NC-SA 2.0 KR material; preserve its [attribution and license](THIRD_PARTY_NOTICES.md). Original fixture prose and names are MIT-licensed.

## Supported subset and exclusions

| Feature | NamuMark profile | Meaning / boundary |
| --- | --- | --- |
| Text | UTF-8, blank-line paragraphs | Nonblank source newline is a line break; retain original-input diagnostic offsets across CRLF normalization. |
| Headings | One to six matched equals signs, spaces inside, complete line: `== Orchard ==` | Preserve level/inline contents. Folded headings excluded; malformed heading-shaped lines diagnosed. |
| Emphasis | Two/three apostrophes for italic/bold, balanced combinations | Preserve emphasis nesting. Balanced/asymmetric nesting: symmetric `'''''x'''''` renders italic outside bold; `'''''x''' y''` closes bold first and renders italic outside bold; `'''''x'' y'''` closes italic first and renders bold outside italic. An italic closer may take the leading two apostrophes of a five-or-more run (leaving the rest to open bold), and a shared five-apostrophe run can close an inner bold/italic opened inside the outer style (e.g. `''italic'''bold-italic'''''`). A four-apostrophe run is bold plus one literal apostrophe and never closes italic (`''''x''''` renders `<strong>'x</strong>'`). Unclosed markers diagnosed. Other formatting excluded. |
| Escape | Backslash before one character | Escape one character, not whole delimiters; trailing backslash literal. Never reinterpret escaped syntax in later stages. |
| Links | `[[Orchard]]`, `[[Orchard\|visit]]`, `[[https://example.org/a\|visit]]` | Same label/destination under independent policy below. Supported inline labels, no nested links/media. |
| Inline literal | Triple braces without newline/directive | Literal monospaced content, no nested evaluation. |
| Literal block | Untyped triple-brace opener/closer on separate lines | Preserve preformatted content/newlines. HTML/wiki/syntax/color/size directives excluded. |
| Lists | Top-level ` * item` and ` 1. item` | One leading space and space after marker. Nested/mixed/continuation lists and other numbering excluded. |
| Break | `[br]` | Explicit line break, the sole supported macro. The macro name matches ASCII case-insensitively (`[BR]`, `[Br]`). |
| Rule | Whole line of four to nine hyphens, optional leading spaces | Emit `----`; three/ten hyphens remain plain text. |
| Categories | `[[분류:Orchards]]`, optional `\|sort key` | Membership/index sort metadata. `[[:분류:Orchards]]` visible link. Blur excluded. |
| Redirect | Standalone `#redirect Orchard` or `#넘겨주기 Orchard` | Metadata target with supported fragment; emit `#redirect`. Diagnose extra nonblank content instead of deleting silently. |

Code mappings have delimiter limits when emitting NamuMark. An inline code value containing a newline or `}}}`, ending in `}`, ending in an odd number of backslashes, or beginning with `#` cannot use the profile's inline triple-brace form safely. Best effort emits escaped visible text and a located conversion loss for the lost code formatting; strict mode rejects it. A preformatted block can retain leading `#` text and blank lines, but content containing the literal closing sequence `}}}` falls back to escaped visible text with loss. These are output-representation limits, distinct from unclosed source delimiters.

Standalone lists at document start are accepted by this converter profile. The snapshot mentions a preceding-content dependency near its comments section; this permissive rule is not a claim about live NamuWiki behavior. Likewise, three post-baseline profile conventions ratified in [#66](https://github.com/librewiki/rWikiText/issues/66)—ASCII case-insensitive `[br]` matching, bare `/` and `/#fragment` child-relative resolution to `context.page_title`, and extended apostrophe emphasis rules (5+ shared closer-run donation and 4-apostrophe bold-plus-literal handling)—are converter-profile decisions covered by contract/inline tests rather than live-NamuWiki renderer evidence.

Tables, including simple rectangular tables, are excluded. Preserve the entire table visibly as escaped literal text with a located loss, rather than reading cells as another dialect. Also excluded: footnotes/references, quotations, media/files/video, includes and other macros, custom/numbered section anchors, HTML/CSS directives, folding, editor comments, advanced namespace/link options, and syntax only in the unprovided advanced/developer help. Recognized excluded constructs receive conversion-loss diagnostics even if their literal text survives. Among single-bracket inline constructs, only recognized macro or footnote forms are diagnosed: footnote `[* …]`; bare macros `date`, `datetime`, `tableofcontents`/`목차`, `footnote`/`각주`, `clearfix`, and `pagecount`; argument-required macros `include`, `age`, `dday`, `youtube`, `nicovideo`, `vimeo`, `navertv`, `ruby`, and `anchor`, which count as macros only when parentheses are present (`br` is supported; a parenthesized `br(...)` is treated as an excluded macro). Latin macro names match ASCII case-insensitively; Korean names match exactly. Ordinary prose such as `[1]` or `[sic]` is literal text and is not diagnosed.

## Six-direction mappings

Each pair below applies in both directions. The supported intersection preserves text, heading levels, emphasis, labels/destinations, breaks, code, list ordering, and rules. Everything outside that intersection needs safe best-effort output plus located conversion loss.

| Pair | NamuMark to destination | Source to NamuMark |
| --- | --- | --- |
| MediaWiki | Equivalent paragraphs/headings/apostrophe emphasis; internal wikilinks; external `[https://… label]`; code/pre; `*`/`#` lists; breaks/rules. Categories become `Category:` and redirects `#REDIRECT [[target]]`. Escape literal wiki syntax. | Parse under existing MediaWiki context/callback rules, then emit NamuMark subset, `분류:` category/sort metadata, and unbracketed redirect. Diagnose source-only meaning such as display titles and transclusion identity even when expanded visible contents survive. |
| GFM | Equivalent paragraphs/ATX headings/emphasis/Markdown links/code spans/fences/lists/hard breaks/rules. Return category/redirect metadata separately and diagnose body-only loss. | Emit NamuMark supported structures. Plain category-looking text remains text, not metadata. Diagnose tables/images/tasks/strike/unsupported HTML/nested lists and other unsupported meaning. |
| HTML | Semantic p/h1–h6/em/strong/a/code/pre/ul/ol/li/br/hr. Return metadata and encode categories/redirects in existing validated page-metadata transport, a rWikiText convention rather than NamuWiki HTML specification. | Import supported semantic structures and validated category/redirect transport; emit NamuMark. Diagnose unsupported meaning-bearing structures/attributes. Arbitrary HTML/data-mw does not establish NamuMark macro identity. |

HTML emphasis tags follow the source format's contract: NamuMark → HTML emits semantic `<strong>` and `<em>` tags (matching the semantic HTML profile above), whereas MediaWiki wikitext → HTML emits `<b>` and `<i>` to match Parsoid's MediaWiki DOM specification. HTML imports accept both `<strong>`/`<b>` and `<em>`/`<i>`.

Translate category namespace spelling for membership and escaped category-page links. Other namespaces retain literal title semantics. NamuMark does not inherit MediaWiki first-letter capitalization, underscore normalization, link trails, or template evaluation. Diagnose mismatches when destination title rules change meaning.

For NamuMark → MediaWiki, a page title containing MediaWiki delimiter characters such as `[]{}|#` cannot be emitted as an active wiki target. Destination namespace/title normalization that changes a NamuMark target also requires an original-input loss location. Unsafe link targets remain visibly escaped literal text; unsafe category/redirect metadata is emitted visibly and escaped rather than activated as MediaWiki syntax, while the result retains the structured metadata. Strict mode rejects these losses. HTML/GFM imports targeting NamuMark use its own title policy: retain exact case, spaces, underscores, and escaped reserved characters instead of first applying MediaWiki title normalization. Validated HTML category/redirect transport likewise retains NamuMark spelling.

## Independent NamuMark context and URL policy

- NamuMark input makes no network requests and invokes no template/page-existence/file callbacks. Includes/media remain excluded even if WikiData provides similarly named resources. Language/time do not expand macros or alter title spelling.
- Internal titles preserve case/spaces/underscores. Portable URLs use `./` plus UTF-8 percent-encoded title and optional fragment; fragment-only links remain `#fragment`. Escape HTML attributes. Decode explicit internal transport once on reverse conversion; arbitrary site URLs do not establish wiki identity.
- `/Child` appends to context.page_title, and bare `/` (including `/#fragment`) resolves to context.page_title. `../` chooses parent at last slash, or current title if none. Missing context is a located conversion loss, never a lookup. Explicit fragments are preserved; automatic `s-N` section anchors excluded because destination numbering differs.
- Active external links allow only absolute http/https with nonempty authority, case-insensitive scheme, no controls/whitespace/quotes/angle brackets/backslashes/brackets. Rejected schemes and protocol-relative URLs stay visibly literal with conversion loss; never reinterpret rejected URL schemes as internal titles.
- Plain HTML-looking NamuMark text is literal/escaped on HTML output. HTML directives are excluded with loss, not executed/imported. No raw-HTML passthrough.
- Expansion limits retain their MediaWiki meaning. NamuMark consumes no template expansion counts/stacks and invokes no wiki callbacks. Its inline parser bounds recursive nesting at 32 levels; deeper inline content falls back to visible text with a located nesting-limit conversion loss, rejected in strict mode. This is a NamuMark parser bound, independent of the caller's MediaWiki expansion budgets. It does not establish a general input-size or memory bound. Malformed input must not panic/loop.

## Located losses and malformed input

Best effort preserves recognized unsupported/malformed constructs visibly as escaped literal text, with ConversionLoss and a reason naming lost meaning/syntax. Literal fallback does not prove intended rendered meaning survived. Locate balanced excluded constructs as a whole; unclosed delimiters from opener to block/input end. Ordinary punctuation/literal-code content is not diagnosed merely for resembling markup.

Spans are half-open UTF-8 byte ranges in original input. Test multibyte prefixes, repeated constructs, and CRLF. For target-only losses where shared representation no longer retains exact location, whole original input is an allowed conservative span; disclose lower precision and never label intermediate-output offsets original-input locations. Metadata-body losses use original category/redirect ranges when available.

Strict mode errors carrying diagnostics whenever conversion loss occurs, including malformed/unsupported syntax and GFM body-only metadata loss. Public Rust API, CLI, C ABI, and bindings agree. Identity/direct GFM–HTML requests still fail before parsing.

## Verification and release gates

Fixture trees/metadata are authored from the pinned snapshot/profile before implementation, with original prose/names/code. Changes require reference/meaning review rather than accepting converter output. Compare semantic HTML allowing incidental heading IDs, attribute order, and block serialization whitespace to differ; retain text/hierarchy/links/metadata.

Exercise all six directions through public Rust API/CLI, plus C ABI/bindings selection/results. Check NamuMark-origin round trips through each destination and round trips starting at each other format. Body-only GFM deliberately loses category/redirect metadata. Check exclusions/malformed/strict/UTF-8 ranges/URLs/no callbacks, and re-run all four existing-direction regressions.

For [#49](https://github.com/librewiki/rWikiText/issues/49), record actual external MediaWiki/Parsoid evidence when available and published-GFM structure checks. Manual-derived NamuMark checks must be labeled as such, never as real renderer results. Unavailable renderer, skipped external job, and unrun platform are not passing evidence. Record candidate commit/commands/reference versions/results/gaps. Full live-NamuWiki compatibility remains unverified until independent renderer or reviewed captured renderings are added.

[#48](https://github.com/librewiki/rWikiText/issues/48) implements this baseline after #47. [First-release scope](https://github.com/librewiki/rWikiText/issues/2) includes all six new directions. [glibc](https://github.com/librewiki/rWikiText/issues/26) and [musl](https://github.com/librewiki/rWikiText/issues/27) checks must use expanded candidate; [publication](https://github.com/librewiki/rWikiText/issues/28) waits for those checks and NamuMark verification. Historical MediaWiki results remain evidence only for recorded revision.
