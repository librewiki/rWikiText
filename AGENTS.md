# rWikiText

rWikiText is a Rust parser for MediaWiki wikitext.

## Work lifecycle

Before starting work, read and follow the [work lifecycle](docs/agents/issue-tracker.md#work-lifecycle) for branches, the `wip` label, commits, and pull requests.

## Parallel work

Delegate tasks that can proceed concurrently to subagents. Use the model and reasoning effort assigned to each role in the main agent's environment.

| Main agent | Documentation, design, task instructions, test authoring, and verification | Code writing and test execution |
| --- | --- | --- |
| ChatGPT (Codex) | GPT-6.1 Sol (High) | GPT-6.1 Terra (High) |
| Antigravity | Gemini 4 Argon (High) | Gemini 4 Hydrogen (High) |

## Agent skills

### Issue tracker

Use GitHub Issues for librewiki/rWikiText. See `docs/agents/issue-tracker.md` before creating, reading, or updating issues.

### Triage labels

Use the default triage labels. See `docs/agents/triage-labels.md` when assigning a triage status.

### Domain docs

Use the single-context layout. See `docs/agents/domain.md` before exploring the parser or making domain decisions.
