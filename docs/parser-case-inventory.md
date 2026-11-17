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

| Status | Meaning |
| --- | --- |
| `dom-matches` | The forward DOM probe matched and produced no diagnostics. |
| `dom-matches-with-diagnostics` | The probe matched and the report records the diagnostics for review. |
| `dom-differs` or `conversion-error` | The probe found a structure difference or conversion error. Review the fixture data, comparison and release behavior for each case. |
| `needs-context` | The probe needs more fixture support, such as a site setting or upstream media fixture. |
| `needs-oracle` | The case needs a suitable forward input and standalone or shared HTML expectation. Some fixtures have only an integrated Parsoid or legacy PHP expectation. |
| `other-mode` | Upstream selects a different test mode. Reverse-conversion and round-trip cases can still apply to the release. |
| `outside-release` | The report gives the specific scope reason listed below. |

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
| `release_parser_cases` | `vendor/wikimedia/parsoid/tests/parser/parserTests.txt` | Explicit namespace, template names supplied as arguments, titles with entities or stripped fragments, commented and multiline titles, multiline and complex arguments, comment-only lines in parameters, nested forwarding, named parameter trimming, positional whitespace, and parameter names with comments, wikitext, blank keys or the name `constructor` |
| `release_parser_cases` | `tests/parser/preprocessor.txt` | `Preprocessor precedence 5: tplarg takes precedence over template` |
| `release_parser_cases` | `tests/parser/parserTests.txt` | `T2529: Template with table, not included at beginning of line` and the list items generated by adjacent template calls in `Implicit newline insertion/sol when expanding templates (T241150/T14974/T2529)` |
| `functions`, `release_parser_cases` | `tests/parser/magicWords.txt` | Ten numeric UTC words including `CURRENTDOW` and `CURRENTWEEK`, four UTC calendar names (`CURRENTDAYNAME`, `CURRENTMONTHABBREV`, `CURRENTMONTHNAME`, `CURRENTMONTHNAMEGEN`), `FULLPAGENAME`, `PAGENAME`, `SUBPAGENAME`, `BASEPAGENAME`, `ROOTPAGENAME`, `NAMESPACE`, and both `PAGELANGUAGE` cases |
| `functions`, `release_parser_cases` | `tests/parser/parserTests.txt` | Three basic padding cases, both padding strip marker cases and three Turkish case conversion cases |
| `release_parser_cases` | `tests/parser/pfeqParserTests.txt` | `Basic usage of {{=}}`, `Using {{Template:=}} doesn't invoke the parser function` |
| `release_parser_cases` | `tests/parser/extTags.txt` | Five core `#tag` cases for `nowiki` and `pre`, including strip markers in tag names, content and attributes |
| `release_parser_cases` | `vendor/wikimedia/parsoid/tests/parser/v3ParserFunctions.txt` | The version 2.x parser-function metadata case with the experimental option disabled |
| `functions`, `release_parser_cases` | `extensions/ParserFunctions/tests/parser/funcsParserTests.txt` | All 12 cases whose inputs exercise the supported `#if`, `#ifeq`, `#switch`, and `#ifexist` functions exclusively. The fixture adapter reports media availability after the caller resolves file redirects. |

The regular `tables`, `metadata`, `wiki_links`, and `parsoid_transclusion` suites also run local regressions based on the upstream fixtures. These cover simple, merged-cell and nested tables, a category with a whitespace-only sort key, plain and styled display titles, simple and colon redirects, image alignment, size and captions, and list transclusion. These independently runnable regressions use local inputs and expectations.

The calendar name checks use the fixed upstream reference time. The API fixtures also check that the names follow standalone Parsoid's English output with Korean and Russian site languages.

## Cases outside the release scope

| Upstream cases | Scope reason |
| --- | --- |
| `funcsParserTests.txt` cases for `#time`, `#timel`, `#timef`, `#timefl`, `#titleparts`, `#iferror`, and `#ifexpr` | These functions belong to the bundled ParserFunctions extension and are outside the release's supported subset. Three mixed cases (`Zero parameters in #if and such`, `Empty condition in #if and such`, `Missing or all-empty outputs`) also invoke `#ifexpr`. The other 12 of the file's 37 cases run against the supported conditional functions. |
| `stringFunctionTests.txt` | `#len`, `#urldecode`, `#pos`, and `#explode` require optional ParserFunctions extension hooks. Core case conversion and padding are checked in `parserTests.txt`. |
| `v3ParserFunctions.txt` experimental cases | Two cases enable experimental Parsoid v3.x function output. The release pin is Parsoid HTML 2.8.0. |
| 104 extension or fixture-hook cases | `annotationParserTests.txt` uses `dummyanno`, `pFragmentHandlerTests.txt` registers test-only functions, `i18nParserTests.txt` uses its i18n test hook, and `timedMediaHandlerParserTests.txt` needs the media extension. The first `encapParserTests.txt` case expands `<pwraptest>` through a template. These require functionality outside [#2's release scope](https://github.com/librewiki/rWikiText/issues/2). The second `encapParserTests.txt` case uses ordinary HTML. The `Leaking fragment marker` case in `pFragmentHandlerTests.txt` uses an ordinary template and requires the integrated oracle. |
| 60 selective serialization cases | The upstream cases run only `selser`, applying a DOM diff and partially reserializing the original wikitext. The release API converts complete documents. Cases with ordinary conversion modes still apply in those modes, including the metadata spoofing case in `selserWrappingParserTests.txt`. |
| 44 pre-save transformation cases | The `pst` option requests a page save operation, including signature insertion and substitution. `pst.txt` also has six cases without that option. `editTests.php` tests edit submission separately from the `.txt` corpus. |
| `Template infinite loop` | The release contract uses explicit errors for configured expansion bounds. `transclusion` exercises depth, invocation count and output size through the public API. MediaWiki's in-page loop marker and tracking categories are runtime-specific outputs. |

Some fixtures, including site-wide counts, revision metadata and user preferences in `magicWords.txt`, depend on MediaWiki runtime data. The release context provides page title, site language, reference time and three data callbacks. Review these fixtures against that contract before deciding scope, since others in the same file use data the context already supplies.

## Remaining parser corpus

The audit still needs case-level review for additional table and paragraph wrapping, list and heading edge cases, transclusion grouping and preprocessing, link and media variants, category and redirect variants, context words and localization.

- `tests/parser/magicWords.txt`, `Magic Word: {{PAGENAMEE}}`. Convert `{{PAGENAMEE}}` from wikitext to HTML with `--page-title 'User:Ævar Arnfjörð Bjarmason'`. The pinned paragraph contains `%C3%86var_Arnfj%C3%B6r%C3%B0_Bjarmason`. rWikiText produces an empty body and reports conversion loss at bytes 0 through 12.
- An unclosed internal link can suppress the line break required by a later template expansion. With `Template:1x` defined as `{{{1}}}`, standalone Parsoid renders `before [[unterminated {{1x|*Text}}` as a paragraph followed by a list. rWikiText retains the expansion in the paragraph.

When template output combines with surrounding block syntax or the output of another invocation, rWikiText can render the expected DOM and report a conversion loss for the original invocation. The positional whitespace case verifies preformatted content and the location of the reported loss. The adjacent list-producing invocations verify the ordered list and both loss locations. Parsoid preserves both invocations in the list's `data-mw`.

The remaining `extTags.txt` cases exercise `#tag` with a bogus tag name and `indicator`. The two indicator cases also expect page-level indicator metadata.
