# Issue tracker

Specs and tickets live in GitHub Issues for `librewiki/rWikiText`. Use `gh` from the repository root. Before creating the first issue in a session, verify the target with `gh repo view --json nameWithOwner --jq .nameWithOwner`.

## Issues

- Publish a spec or ticket with `gh issue create --title "..." --body-file <path>`. Use a temporary Markdown file for the body.
- Fetch a ticket with `gh issue view <number> --json number,title,body,labels,comments`.
- List tickets with `gh issue list --state open --json number,title,body,labels,comments`. Filter by state or `--label` as needed.
- Apply or remove a triage label with `gh issue edit <number> --add-label <label>` or `--remove-label <label>`. Use the names in `triage-labels.md`.
- Post an explanation with `gh issue comment <number> --body-file <path>`, then close with `gh issue close <number>`.

## Work lifecycle

### Start work

1. Read the issue and its acceptance criteria when the work has an associated issue.
2. Start on a new branch. Use `feature/<slug>` for regular work, `release/<version>` to prepare a release, and `hotfix/<slug>` for an urgent production fix. Start feature and release branches from `develop` when it exists. Until then, use `main`. Start hotfix branches from `main`.
3. Mark the associated issue in progress before implementation with `gh issue edit <number> --add-label wip`.

### End work

1. When the work session ends, remove `wip` from the associated issue with `gh issue edit <number> --remove-label wip`. Do this for completed, blocked, and interrupted work.
2. Commit the changes made for the task. Use a categorized subject in the form `<type>: <summary>`, with types such as `feat`, `fix`, `docs`, `refactor`, `test`, and `chore`. After a blank line, describe the changes and the verification performed. Include the names of the tools and models actually used, in a form such as `Tools and models: Codex (GPT-6.1 Sol), Git, GitHub CLI`.
3. When all acceptance criteria are verified, close the issue, push the work branch, and open a GitHub pull request (PR) in that order. Use the GitFlow target branch and reference the issue as `Refs #<number>` in the PR body. If any acceptance criterion remains unmet or unverified, keep the issue open.

## Sub-issues and dependencies

- Link a sub-issue with `gh issue edit <parent> --add-sub-issue <child>` when supported. With older `gh` versions, use `gh api --method POST 'repos/{owner}/{repo}/issues/<parent>/sub_issues' -F sub_issue_id=<child-database-id>`. Get the database ID with `gh api 'repos/{owner}/{repo}/issues/<child>' --jq .id`. If sub-issues are unavailable, add `Part of #<parent>` to the child body.
- Record a blocker with `gh issue edit <child> --add-blocked-by <blocker>` when supported. With older `gh` versions, use `gh api --method POST 'repos/{owner}/{repo}/issues/<child>/dependencies/blocked_by' -F issue_id=<blocker-database-id>`. If dependencies are unavailable, add a `Blocked by: #<number>` line to the ticket body.
- Check open blockers with `gh api 'repos/{owner}/{repo}/issues/<number>' --jq .issue_dependencies_summary.blocked_by`. If dependencies are recorded in the body, check the state of every listed blocker. A ticket is unblocked when all of its blockers are closed.

## Pull requests as a triage surface

**PRs as a request surface: no.**

## Wayfinding

- Keep notes, decisions, and open questions in one map issue with the `wayfinder:map` label.
- Link each ticket as a sub-issue and apply its `wayfinder:<type>` label (`research`, `prototype`, `grilling`, or `task`). If sub-issues are unavailable, also link the child from a task list in the map body.
- Choose the first open, unassigned child with no open blockers, in map order. Claim it with `gh issue edit <number> --add-assignee @me` before starting work.
- Post the result on the child issue and close it. Add a short summary and a link to the result in the map's decisions section.
