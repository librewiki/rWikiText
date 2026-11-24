# NamuMark conversion plan

NamuMark (나무마크) is a planned input and output format for the first public release. Current code supports only MediaWiki wikitext ↔ GFM and MediaWiki wikitext ↔ HTML. There is no NamuMark Rust format variant, CLI token, or C ABI format value yet. Existing examples and compatibility results describe MediaWiki support.

## Planned directions

| Format pair | Status |
| --- | --- |
| NamuMark ↔ MediaWiki wikitext | Planned |
| NamuMark ↔ GFM | Planned |
| NamuMark ↔ HTML | Planned |

These add six directions to the existing four. Direct GFM ↔ HTML and identity conversions remain outside the contract. Conversion preserves document meaning: rendered content, structure, and page metadata where the destination can represent them. Unrepresentable meaning requires a conversion-loss diagnostic with its original input byte range and reason; strict mode rejects conversion loss.

## Syntax and compatibility reference

Use the user-supplied [NamuWiki syntax help snapshot](namumark_manual_r927_20261124.txt) as the starting reference. The filename identifies revision r927 dated 2026-11-24; its SHA-256 and attribution are recorded in [third-party notices](THIRD_PARTY_NOTICES.md). Direct retrieval of the [source page](https://namu.wiki/w/%EB%82%98%EB%AC%B4%EC%9C%84%ED%82%A4:%EB%AC%B8%EB%B2%95%20%EB%8F%84%EC%9B%80%EB%A7%90) failed, so the online revision has not been independently checked. No renderer or executable NamuMark compatibility baseline is pinned.

The snapshot covers text formatting and colors, links, redirects, images/video, headings, lists, footnotes, quotations, horizontal rules, macros and includes, tables, folding, categories, escaping, and comments. It links to advanced/developer help that is not part of this snapshot. This inventory identifies specification work, not implemented support.

Before implementation, record a reproducible reference revision or retrieval date, permitted reference fixtures, and an independent expected-output strategy. Inventory text and escaping, headings, links and files, lists, tables, formatting, code, footnotes, macros/includes, and metadata; agree the supported first-release subset and explicit exclusions. These are investigation categories, not support claims.

The supplied help file is CC BY-NC-SA 2.0 KR material, separate from the project's MIT-licensed code. Preserve it as reference material with its attribution and license notice. Track any copied or adapted examples under their applicable terms; write original test cases where possible and record their provenance.

Define mappings and losses for every supported feature in all six directions. Decide how NamuMark page context, lookups, macros/includes, HTML, and URLs are handled from the NamuMark reference. MediaWiki template semantics, Parsoid metadata, and MediaWiki HTML policy alone do not specify NamuMark behavior. Parsoid continues to verify the MediaWiki side; the published GFM specification remains the GFM reference.

## Implementation and release work

| Work | Issue | Prerequisite |
| --- | --- | --- |
| Define syntax subset, mappings, diagnostics, reference, and fixtures | [#47](https://github.com/librewiki/rWikiText/issues/47) | Review the supplied snapshot and reference provenance |
| Implement shared Rust conversion, API/CLI selection, ABI/bindings, and tests | [#48](https://github.com/librewiki/rWikiText/issues/48) | #47 |
| Verify compatibility, round trips, regressions, and release candidate | [#49](https://github.com/librewiki/rWikiText/issues/49) | #48 |

Implementation must preserve existing format identifiers and ABI values, version incompatible ABI changes, update format-pair restrictions, and expose the agreed behavior in every supported binding. Actual NamuMark identifiers are chosen during implementation.

Verification must exercise all six directions through the Rust API and CLI, round trips starting at NamuMark and at each other format, malformed/unsupported input, located loss diagnostics, strict mode, and agreed context/data limits. Compare rendered structure and meaning-bearing metadata using the appropriate format reference. Re-run MediaWiki regressions and record the exact candidate revision, references, commands, results, exclusions, and unresolved gaps in the compatibility documentation.

The [first-release spec](https://github.com/librewiki/rWikiText/issues/2) includes this expanded scope. The [glibc](https://github.com/librewiki/rWikiText/issues/26) and [musl](https://github.com/librewiki/rWikiText/issues/27) binding checks must run against the expanded candidate. [Publication](https://github.com/librewiki/rWikiText/issues/28) waits for NamuMark verification and those candidate checks. Historical MediaWiki results remain evidence for their recorded revision only.
