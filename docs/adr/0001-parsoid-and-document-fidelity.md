# Use Parsoid as the wikitext compatibility reference

rWikiText supports Wikitext ↔ GFM and Wikitext ↔ HTML. MediaWiki syntax documentation guides implementation, and Parsoid decides behavior where the documentation and rendering differ. Round-trip fidelity means preserving document meaning, with flexibility in the spelling and whitespace of equivalent wikitext.

## Conversion contract

- The first release covers MediaWiki core syntax, including templates, template parameters, and core parser functions. Prioritize extension features in later work based on the documents callers need to convert.
- Emit HTML with Parsoid metadata such as `data-mw`. Accept both general HTML and Parsoid HTML, and use validated metadata to recover constructs such as template calls. [Parsoid HTML specification](https://www.mediawiki.org/wiki/Parsoid/MediaWiki_DOM_spec)
- Use the published GFM specification for compatibility. Prefer native GFM syntax and use GFM-permitted HTML for structures such as tables with merged cells. GitHub-hosted rendering applies additional post-processing. [GFM specification](https://github.github.com/gfm/)
- Return structured page metadata, including categories, redirects, and display titles. Encode it in the converted document where the format supports it. Report conversion losses with a location and reason. A result can carry metadata separately from its GFM body and report that storing the body alone loses that information. Strict mode treats conversion loss as an error.
- Apply MediaWiki's allowed HTML elements, attributes, and URL policies to embedded or imported HTML. Diagnose removed content and validate Parsoid metadata before using it. Bound template expansion depth, expansion count, and output size.

## Verification

At implementation kickoff, pin the exact stable MediaWiki release and its Parsoid HTML specification version. Run the applicable official parser tests for the supported scope. Compare normalized HTML DOM and metadata that carries meaning. Verify reverse conversions by rendering their wikitext and comparing document meaning. Render GFM under the published specification and compare document structure. [MediaWiki parser tests](https://www.mediawiki.org/wiki/Manual%3AParser_tests)

Start with paragraphs, headings, links, and lists. Exercise all four conversion directions and both round trips in the Rust library and CLI before extending coverage to Parsoid metadata, template callbacks, the rest of core syntax, and language bindings.
