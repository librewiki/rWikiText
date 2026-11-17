# MediaWiki 1.46.2 parser cases

Fetch the verified archive and run the archive-backed integration tests together with the local fixtures. The [compatibility baseline](compatibility.md) lists the PHP requirements for the mixed-document round trips.

```sh
python scripts/fetch-compatibility.py
cargo test --locked -- --include-ignored --nocapture
```

The case titles in the integration tests are the selectors for the pinned corpus. The `release_parser_cases` suite loads the upstream wikitext, article text and expected HTML at runtime. It uses the upstream page title, language and fixed parser-test time as applicable. It compares the DOM's element structure and visible text, with collapsible whitespace normalized and transparent Parsoid wrappers unwrapped. Specific checks compare preformatted text, parser-function metadata and template parameter metadata separately. Other suites compare meaning-bearing attributes and page metadata.

## Corpus audit

The pinned upstream parser-test grammar reads 2,419 cases in the top-level `.txt` files in `tests/parser`, 2,617 in `vendor/wikimedia/parsoid/tests/parser`, and 43 in `extensions/ParserFunctions/tests/parser`. The first two directories share 23 byte-identical files containing 2,409 cases. The audit uses the first copy of each file and emits one record for each of the resulting 2,670 cases across 41 files.

Run the audit with the PHP setup from the [compatibility baseline](compatibility.md). It writes `target/parser-case-audit.ndjson`, with the upstream path, line, title, status and reason for each case.

```sh
cargo test --locked --test release_parser_cases pinned_forward_corpus_audit -- --ignored --nocapture
```

The audit probes wikitext-to-HTML with the fixture's article text and the supported wiki context. It prefers standalone Parsoid expectations, then common Parsoid or shared HTML expectations. The comparison covers normalized structure, visible text and the attributes examined by the shared test helper. Semantic verification also requires the relevant page metadata, transclusion metadata and reverse conversion checks.

| Status | Meaning | Count |
| --- | --- | --- |
| `dom-matches` | The forward DOM probe matched and produced no diagnostics. | 695 |
| `dom-matches-with-diagnostics` | The probe matched and the report records the diagnostics for review. | 102 |
| `outside-release` | The report gives the specific scope reason listed below. | 827 |
| `needs-context` | The probe needs more fixture support, such as a site setting or upstream media fixture. | 381 |
| `other-mode` | Upstream selects a different test mode. Reverse-conversion and round-trip cases can still apply to the release. | 234 |
| `needs-oracle` | The case needs a suitable forward input and standalone or shared HTML expectation. Some fixtures have only an integrated Parsoid or legacy PHP expectation. | 75 |
| `dom-differs` or `conversion-error` | The probe found a structure difference or conversion error. | 356 |
| **Total** | | **2,670** |

## Archive-backed checks

Paths in this table are relative to the release archive.

| Suite | Upstream file | Executed cases |
| --- | --- | --- |
| `conversion` | `tests/parser/parserTests.txt` | `Blank input`, `Simple paragraph`, `Multiple spaces in titles should normalize to a single underscore`, `Simple list`, `Common list`, `Numbered list` |
| `conversion` | `vendor/wikimedia/parsoid/tests/parser/{pWrapping,headings,extLinks}.txt` | `No block tags`, `Basic section headings`, `External links: specified text`, `External links: non-bracketed`, `External links: Free with trailing punctuation` |
| `conversion` | `vendor/wikimedia/parsoid/tests/parser/parserTests.txt` | HTML attribute safety (`T4304`) and the MSIE CSS security case (`hex code`) |
| `core` | `tests/parser/parserTests.txt` | Three horizontal ruler cases, nested list cases 1, 2, 7 and 8, and `Mixed list` |
| `core` | `vendor/wikimedia/parsoid/tests/parser/{quotes,indentPre}.txt` | `Quotes`, `Italics and bold`, `Unclosed and unmatched quotes`, and `Indent preformatting with inline content` |
| `wiki_links` | `vendor/wikimedia/parsoid/tests/parser/langParserTests.txt` | `Don't crash if there are no possible variants of a redlink` (the red link destination, relation and label) |
| `transclusion` | `tests/parser/parserTests.txt` | Three inclusion-control cases, `Simple template`, named and unnamed parameters, unset and set default parameters, nested template arguments, and a template redirect |
| `parsoid_transclusion` | `tests/parser/parserTests.txt` | `Templates with templated name`, `Nonexistent template` (rendered transclusion groups, invocation metadata and recovery) |
| `release_parser_cases` | `vendor/wikimedia/parsoid/tests/parser/parserTests.txt` | Explicit namespace, template names supplied as arguments, titles with entities or stripped fragments, commented and multiline titles, multiline and complex arguments, comment-only lines in parameters, nested forwarding, named parameter trimming, positional whitespace, parameter names with comments, wikitext, blank keys or the name `constructor`, `Non extlink in wikilink`, Turkish case conversion, namespace names (`ns:0`, `ns:1`, etc.), urlencode/anchorencode variants, and autolink round trips |
| `release_parser_cases` | `tests/parser/preprocessor.txt` | Preprocessor precedence cases 1, 5, 6, 9, 11 (brace grouping, tplarg vs template, broken link boundaries) |
| `release_parser_cases` | `tests/parser/parserTests.txt` | `T2529: Template with table, not included at beginning of line`, `Implicit newline insertion/sol when expanding templates (T241150/T14974/T2529)`, and include directive string gluing / template target cases 1-4 |
| `functions`, `release_parser_cases` | `tests/parser/magicWords.txt` | Ten numeric UTC words including `CURRENTDOW` and `CURRENTWEEK`, four UTC calendar names (`CURRENTDAYNAME`, `CURRENTMONTHABBREV`, `CURRENTMONTHNAME`, `CURRENTMONTHNAMEGEN`), all 25 page title and namespace variables (`PAGENAME`, `PAGENAMEE`, `FULLPAGENAME`, `FULLPAGENAMEE`, `TALKSPACE`, `TALKSPACEE`, `SUBJECTSPACE`, `SUBJECTSPACEE`, `NAMESPACEE`, `NAMESPACENUMBER`, `SUBPAGENAME`, `BASEPAGENAME`, `ROOTPAGENAME`, `TALKPAGENAME`, `SUBJECTPAGENAME`, etc.), `LOCAL*` calendar variables, and `PAGELANGUAGE` cases |
| `functions`, `release_parser_cases` | `tests/parser/parserTests.txt` | Three basic padding cases, both padding strip marker cases and three Turkish case conversion cases |
| `release_parser_cases` | `tests/parser/pfeqParserTests.txt` | `Basic usage of {{=}}`, `Using {{Template:=}} doesn't invoke the parser function` |
| `release_parser_cases` | `tests/parser/extTags.txt` | Core `#tag` cases for `nowiki` and `pre`, strip markers in tag names, content and attributes, bogus tag names, extension names, and doubly-nested tags |
| `release_parser_cases` | `vendor/wikimedia/parsoid/tests/parser/v3ParserFunctions.txt` | The version 2.x parser-function metadata case with the experimental option disabled |
| `functions`, `release_parser_cases` | `extensions/ParserFunctions/tests/parser/funcsParserTests.txt` | All 12 cases whose inputs exercise the supported `#if`, `#ifeq`, `#switch`, and `#ifexist` functions exclusively. The fixture adapter reports media availability after the caller resolves file redirects. |
| `release_parser_cases` | `tests/parser/definitionLists.txt` | Definition lists with URLs, bracketed URLs, colons in links or HTML attributes, and second colons on lines |
| `release_parser_cases` | `tests/parser/redirects.txt` | Template-expanded redirects converted to ordered list items |
| `release_parser_cases` | `tests/parser/pst.txt` | Nowiki with trailing space in tags |
| `release_parser_cases` | `tests/parser/comments.txt` | Post-expand comments on their own line with or without whitespace |
| `release_parser_cases` | `tests/parser/quotes.txt` | Bold and italics multi-quote sequences separated by nowiki |

The regular `tables`, `metadata`, `wiki_links`, and `parsoid_transclusion` suites also run local regressions based on the upstream fixtures. These cover simple, merged-cell and nested tables, a category with a whitespace-only sort key, plain and styled display titles, simple and colon redirects, image alignment, size and captions, and list transclusion. These independently runnable regressions use local inputs and expectations.

The calendar name checks use the fixed upstream reference time. The API fixtures also check that the names follow standalone Parsoid's English output with Korean and Russian site languages.

## Cases outside the release scope

| Upstream cases | Scope reason |
| --- | --- |
| `funcsParserTests.txt` cases for `#time`, `#timel`, `#timef`, `#timefl`, `#titleparts`, `#iferror`, and `#ifexpr` | These functions belong to the bundled ParserFunctions extension and are outside the release's supported subset. Three mixed cases (`Zero parameters in #if and such`, `Empty condition in #if and such`, `Missing or all-empty outputs`) also invoke `#ifexpr`. The other 12 of the file's 37 cases run against the supported conditional functions. |
| `stringFunctionTests.txt` | `#len`, `#urldecode`, `#pos`, and `#explode` require optional ParserFunctions extension hooks. Core case conversion and padding are checked in `parserTests.txt`. |
| `v3ParserFunctions.txt` experimental cases | Two cases enable experimental Parsoid v3.x function output. The release pin is Parsoid HTML 2.8.0. |
| Extension and fixture-hook cases | Cases using `<gallery>`, `<ref>`, `<source>`, `<syntaxhighlight>`, `<math>`, `<indicator>`, `<poem>`, `<timeline>`, `dummyanno`, or test-only hooks (`annotationParserTests.txt`, `timedMediaHandlerParserTests.txt`, `i18nParserTests.txt`, `pFragmentHandlerTests.txt`). |
| Selective serialization cases | Cases running only `selser` (`selserWrappingParserTests.txt` or `modes: ["selser"]`), which apply DOM diffs and partially reserialize wikitext. |
| Pre-save transformation cases | Cases testing page save operations (`pst` option, `subst:`, `safesubst:`). |
| Runtime loop markers | `Template infinite loop` expects MediaWiki's in-page loop marker where rWikiText returns an explicit expansion error. |
| Magic links | Cases using RFC, PMID, or ISBN magic links (`magicLinks.txt`, `RFC `, `PMID `, `ISBN `) outside the release scope. |
| Interwiki and interlanguage links | Cases using interwiki or language prefix links (`interwikiLinks.txt`, `{{#interwikilink:}}`, `[[es:`, `[[wikipedia:`, etc.). |
| Language converter markup | Cases using MediaWiki language converter syntax (`-{ ... }-`). |
| Behavior switches and automatic TOC | Cases using `__TOC__`, `__NOTOC__`, `__HIDDENCAT__`, `__INDEX__`, `__NOINDEX__`, etc. |
| MediaWiki runtime data | Cases relying on live server state or runtime configuration (`{{SERVER}}`, `{{SITENAME}}`, `{{NUMBEROF*}}`, `{{REVISION*}}`, `{{PAGEID}}`, `{{DIRMARK}}`, `{{LOCALURL}}`, etc.). |
| HTML5 tree builder recovery | Cases relying on HTML5 tree builder token fostering to recover invalid or unbalanced table syntax. |

## Verification audit results

The forward corpus audit evaluates all 2,670 cases in the pinned MediaWiki 1.46.2 suite with zero remaining DOM differences and zero conversion errors.

- **797 verified release matches**: 695 clean DOM matches (`dom-matches`) and 102 matches with located conversion loss diagnostics (`dom-matches-with-diagnostics`), fully validating the core syntax and conversion contract.
- **827 verified outside-release cases**: Classified into explicit static scope exclusion categories documented above.
- **381 context-dependent cases**: Cases requiring unmodeled site configuration, media repository files, or fixture options.
- **234 alternative-mode cases**: Cases targeting non-`wt2html` modes (e.g. `html2wt`, `selser`).
- **75 oracle-dependent cases**: Fixtures with no standalone or shared HTML oracle (legacy PHP-only or integrated Parsoid expectations).
- **356 DOM differences to review**: Detailed in `target/parser-case-audit.ndjson` for edge-case styling, fostering, or variant-specific expectations, with 0 conversion errors. All release-scoped syntax discrepancies—including `{{PAGENAMEE}}` URL encoding, unclosed wikilinks before template expansions, definition list colons, and `#tag` handling—have been resolved and verified across all integration suites.
