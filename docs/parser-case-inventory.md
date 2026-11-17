# MediaWiki 1.46.2 parser cases

Fetch the verified archive and run the archive-backed integration tests together with the local fixtures. The [compatibility baseline](compatibility.md) lists the PHP requirements for the mixed-document round trips.

```sh
python scripts/fetch-compatibility.py
cargo test --locked -- --include-ignored --nocapture
```

The case titles in the integration tests are the selectors for the pinned corpus. The `release_parser_cases` suite loads the upstream wikitext, article text and expected HTML at runtime. It uses the upstream page title, language and fixed parser-test time as applicable. It compares the DOM's element structure and visible text, with collapsible whitespace normalized and transparent Parsoid wrappers unwrapped. Specific checks compare preformatted text, parser-function metadata and template parameter metadata separately. Other suites compare meaning-bearing attributes and page metadata.

## Archive-backed checks

Paths are relative to the release archive. Counting `!! test` markers in top-level `.txt` files yields 2,419 cases in `tests/parser`, 2,617 in `vendor/wikimedia/parsoid/tests/parser`, and 43 in `extensions/ParserFunctions/tests/parser`. The first two directories share 23 byte-identical files containing 2,409 cases. The three directories contain 2,670 cases after those duplicate copies are removed, including fixtures for operations outside this release.

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
| `release_parser_cases` | `vendor/wikimedia/parsoid/tests/parser/parserTests.txt` | Explicit template namespace, template names passed as three forms of arguments, a multiline argument, complex arguments, passing an argument to another template, named parameter trimming, comments within parameter names, and positional whitespace |
| `functions`, `release_parser_cases` | `tests/parser/magicWords.txt` | Ten numeric UTC words including `CURRENTDOW` and `CURRENTWEEK`, `FULLPAGENAME`, `PAGENAME`, `SUBPAGENAME`, `BASEPAGENAME`, `ROOTPAGENAME`, `NAMESPACE`, and both `PAGELANGUAGE` cases |
| `functions`, `release_parser_cases` | `tests/parser/parserTests.txt` | Three basic padding cases, both padding strip marker cases and three Turkish case conversion cases |
| `release_parser_cases` | `tests/parser/pfeqParserTests.txt` | `Basic usage of {{=}}`, `Using {{Template:=}} doesn't invoke the parser function` |
| `release_parser_cases` | `vendor/wikimedia/parsoid/tests/parser/v3ParserFunctions.txt` | The version 2.x parser-function metadata case with the experimental option disabled |
| `functions`, `release_parser_cases` | `extensions/ParserFunctions/tests/parser/funcsParserTests.txt` | All 12 cases whose inputs exercise the supported `#if`, `#ifeq`, `#switch`, and `#ifexist` functions exclusively. The fixture adapter reports media availability after the caller resolves file redirects. |

The regular `tables`, `metadata`, `wiki_links`, and `parsoid_transclusion` suites also run local regressions based on the upstream fixtures. These cover simple, merged-cell and nested tables, a category with a whitespace-only sort key, plain and styled display titles, simple and colon redirects, image alignment, size and captions, and list transclusion. These independently runnable regressions use local inputs and expectations.

## Cases outside the release checks

| Upstream cases | Scope reason |
| --- | --- |
| `funcsParserTests.txt` cases for `#time`, `#timel`, `#timef`, `#timefl`, `#titleparts`, `#iferror`, and `#ifexpr` | These functions belong to the bundled ParserFunctions extension and are outside the release's supported subset. Three mixed cases (`Zero parameters in #if and such`, `Empty condition in #if and such`, `Missing or all-empty outputs`) also invoke `#ifexpr`. The other 12 of the file's 37 cases run against the supported conditional functions. |
| `stringFunctionTests.txt` | `#len`, `#urldecode`, `#pos`, and `#explode` require optional ParserFunctions extension hooks. Core case conversion and padding are checked in `parserTests.txt`. |
| `v3ParserFunctions.txt` experimental cases | Two cases enable experimental Parsoid v3.x function output. The release pin is Parsoid HTML 2.8.0. |
| Test-only extension tags such as `<pwraptest>` in `encapParserTests.txt`, annotations from `dummyanno`, and `timedMediaHandlerParserTests.txt` | These depend on extension registration, extension rendering, or extension-specific media support outside [#2's release scope](https://github.com/librewiki/rWikiText/issues/2). The second `encapParserTests.txt` case uses ordinary HTML and remains applicable. |
| Selective serialization (`selser`) and explicit edit scripts, including `domNormalizerTests.txt` and `selserWrappingParserTests.txt` | The release API converts complete documents. Upstream edit scripts exercise applying a DOM diff and partially reserializing the original wikitext. A case that also supplies an ordinary document conversion expectation can still apply in that mode. |
| `pst.txt` pre-save transform expectations and `editTests.php` | These test edit submission or saving a page, including signature insertion and substitution at save time. The release API accepts and returns document text without a save operation. |
| Revision metadata, user preferences, site-wide counts and server configuration in `magicWords.txt` | The reference harness has a wiki database, revision records and user settings. The release context supplies the page title, site language, reference time and the three lookup callbacks. Fixture expectations for data outside that contract need a MediaWiki runtime. |
| `Template infinite loop` and fixture-specific MediaWiki limit tracking output | The release contract uses explicit errors for configured expansion bounds. `transclusion` exercises depth, invocation count and output size through the public API. MediaWiki's in-page loop marker and tracking categories are runtime-specific outputs. |

## Remaining parser corpus

Many cases in the pinned corpus still need applicability review and execution. This includes additional table and paragraph wrapping, list and heading edge cases, transclusion grouping and preprocessing, link and media variants, category and redirect variants, other context words and localization, and remaining core constructs in `parserTests.txt`.

The pinned `Magic Word: {{CURRENTDAYNAME}}` case in `tests/parser/magicWords.txt` demonstrates a remaining core gap. Convert `{{CURRENTDAYNAME}}` from wikitext to HTML with `--site-language en --reference-time 1970-01-01T00:02:03Z`. The expected paragraph contains `Thursday`. rWikiText reports a conversion loss and produces an empty body.

When a template invocation combines with surrounding block syntax, rWikiText can render the expected DOM and report that the output cannot recover the invocation. The positional whitespace case verifies the preformatted content and the location of the reported loss.
