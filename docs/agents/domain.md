# Domain docs

rWikiText uses a single document-conversion context for MediaWiki wikitext, NamuMark, GFM, and HTML. The documented NamuMark first-release profile and its six conversion directions are implemented; remaining NamuMark syntax and live-NamuWiki renderer verification are future scope.

- `GLOSSARY.md` at the repository root holds the project's domain terms. Read it before exploring the parser and use its terms in issues, proposals, hypotheses, and tests.
- `docs/adr/` holds architecture decision records. Read the ADRs relevant to the work and identify any existing decision a proposal would reopen.
- For NamuMark work, read `docs/namumark.md` and its linked issues before choosing syntax or compatibility behavior. Use an independent NamuMark reference; Parsoid remains the MediaWiki reference. Keep planned directions and syntax distinct from implemented formats, including the NamuMark first-release profile, in documentation and issue results.
- When using the supplied NamuWiki help or examples derived from it, preserve the source and license information in `docs/THIRD_PARTY_NOTICES.md` and record fixture provenance.

Continue with the available documentation. Use the `domain-modeling` skill to record terms and decisions as they are agreed.
