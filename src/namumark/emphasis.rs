use super::{
    inline::InlineParser,
    scan::{scan_unescaped, ScanAction},
};
use crate::Inline;
use std::borrow::Cow;

/// Parse an emphasis construct whose input starts at an apostrophe run.
///
/// Two apostrophes are italic, three or four are bold (a fourth opener or
/// closer apostrophe stays literal), and five or more combine both. A combined
/// opener uses an earlier asymmetric closer pair (`'''` then `''`, or `''`
/// then `'''`) when one closes cleanly before the first five-or-more run on
/// the same line; otherwise it prefers the first run of five or more
/// apostrophes as the symmetric closer so nested short markers stay inside
/// the body, and falls back to the earliest short run when no 5+ run exists.
/// Symmetric closers nest bold inside italic; a two-apostrophe closer closes
/// italic first, so bold wraps the remainder, and a three- or four-apostrophe
/// closer closes bold first, so italic wraps the remainder. Every closer is
/// located before any recursive parsing so a failure never emits nested
/// diagnostics; on failure the whole construct is left to the caller's literal
/// fallback.
pub(super) fn parse_emphasis(
    parser: &mut InlineParser<'_, '_>,
    input: &str,
    base: usize,
    depth: usize,
) -> Result<(Vec<Inline>, usize), ()> {
    let opener = apostrophe_run(input);
    if opener == 2 {
        return parse_single_style(parser, input, base, depth, false);
    }
    if opener == 3 || opener == 4 {
        return parse_single_style(parser, input, base, depth, true);
    }

    // Five or more apostrophes combine italic and bold. If an earlier
    // asymmetric closer pair closes cleanly before the first 5+ run on
    // the line, take it so a later combined opener is not misread as a
    // symmetric closer; otherwise prefer the first 5+ run so nested
    // short markers (`''`, `'''`) stay in the body.
    let body_start = opener;
    let (closer_offset, closer_run) = match find_run_at_least(&input[body_start..], 5) {
        Some((sym_offset, sym_run)) => {
            find_asymmetric_closer_pair(&input[body_start..body_start + sym_offset])
                .unwrap_or((sym_offset, sym_run))
        }
        None => find_run_at_least(&input[body_start..], 2).ok_or(())?,
    };
    let body_end = body_start + closer_offset;
    let (node, consumed) = if closer_run >= 5 {
        // Symmetric combined form: italic outside bold.
        let children = parser.parse(&input[body_start..body_end], base + body_start, depth + 2);
        let node = Inline::Emphasis {
            strong: false,
            children: vec![Inline::Emphasis {
                strong: true,
                children,
            }],
        };
        // Extra closer-run apostrophes are re-lexed by the caller.
        (node, body_start + closer_offset + 5)
    } else {
        // A two-apostrophe closer closes italic first so bold wraps the
        // remainder; a three- or four-apostrophe closer closes bold first so
        // italic wraps the remainder.
        parse_asymmetric(
            parser,
            input,
            base,
            depth,
            body_start,
            body_end,
            closer_run != 2,
        )?
    };
    let mut nodes = Vec::new();
    // Any opener apostrophes beyond the combined marker stay literal.
    if opener > 5 {
        nodes.push(Inline::Text("'".repeat(opener - 5)));
    }
    nodes.push(node);
    Ok((nodes, consumed))
}

/// Parse a single-style emphasis span (italic when `strong == false`, bold when
/// `strong == true`), donating a virtual closer from a shared 5+ run when the
/// body holds an unmatched inner emphasis opener.
fn parse_single_style(
    parser: &mut InlineParser<'_, '_>,
    input: &str,
    base: usize,
    depth: usize,
    strong: bool,
) -> Result<(Vec<Inline>, usize), ()> {
    let (width, inner_width, virtual_closer, closer) = if strong {
        (3, 2, "''", find_run_at_least(&input[3..], 3))
    } else {
        (2, 3, "'''", find_italic_closer(&input[2..]))
    };
    let (closer_offset, closer_run) = closer.ok_or(())?;
    let body_end = width + closer_offset;
    let raw_body = &input[width..body_end];
    // A five-or-more run can donate its leading `width` apostrophes to close
    // this span; if the body holds an unmatched inner opener, a virtual closer
    // closes it and any remainder of the run is re-lexed by the caller.
    let append = if closer_run >= 5
        && simple_emphasis_body(raw_body)
        && odd_emphasis_runs(raw_body, inner_width)
    {
        virtual_closer
    } else {
        ""
    };
    let body: Cow<'_, str> = if append.is_empty() {
        Cow::Borrowed(raw_body)
    } else {
        Cow::Owned(format!("{raw_body}{append}"))
    };
    let children = parser.parse(&body, base + width, depth + 1);
    let node = Inline::Emphasis { strong, children };
    let consumed = body_end + append.len() + width;
    Ok((vec![node], consumed))
}

/// Parse an asymmetric combined emphasis span where one style closes at
/// `body_end` and the other style wraps the remainder until its own closer.
fn parse_asymmetric(
    parser: &mut InlineParser<'_, '_>,
    input: &str,
    base: usize,
    depth: usize,
    body_start: usize,
    body_end: usize,
    inner_strong: bool,
) -> Result<(Inline, usize), ()> {
    let (inner_close_len, outer_close_len) = if inner_strong { (3, 2) } else { (2, 3) };
    let tail_start = body_end + inner_close_len;
    let tail_offset = if inner_strong {
        find_exact_run(&input[tail_start..], 2)
    } else {
        find_run_at_least(&input[tail_start..], 3).map(|(offset, _)| offset)
    }
    .ok_or(())?;
    let tail_end = tail_start + tail_offset;
    let inner = parser.parse(&input[body_start..body_end], base + body_start, depth + 2);
    let tail = parser.parse(&input[tail_start..tail_end], base + tail_start, depth + 1);
    let mut children = vec![Inline::Emphasis {
        strong: inner_strong,
        children: inner,
    }];
    children.extend(tail);
    let node = Inline::Emphasis {
        strong: !inner_strong,
        children,
    };
    Ok((node, tail_end + outer_close_len))
}

/// Locate an asymmetric closer pair (`'''`/`''''` then `''`, or `''` then
/// `'''`+) within `before_sym` when both short-marker parities are odd,
/// indicating the opening 5+ run closes asymmetrically before a later 5+ run.
fn find_asymmetric_closer_pair(before_sym: &str) -> Option<(usize, usize)> {
    if !(odd_emphasis_runs(before_sym, 2) && odd_emphasis_runs(before_sym, 3)) {
        return None;
    }
    let (offset, run) = find_run_at_least(before_sym, 2)?;
    if run == 2 {
        find_run_at_least(&before_sym[offset + 2..], 3)?;
        Some((offset, run))
    } else if run == 3 || run == 4 {
        find_exact_run(&before_sym[offset + 3..], 2)?;
        Some((offset, run))
    } else {
        None
    }
}

/// Number of leading apostrophes in `input`.
pub(super) fn apostrophe_run(input: &str) -> usize {
    input.bytes().take_while(|&byte| byte == b'\'').count()
}

/// Locate the first unescaped run of exactly `n` apostrophes. Runs of any
/// other length are skipped whole so a longer run never closes a shorter one.
fn find_exact_run(input: &str, n: usize) -> Option<usize> {
    find_apostrophe_run(input, |run| run == n).map(|(offset, _)| offset)
}

/// Locate the first unescaped run of at least `n` apostrophes, returning the
/// offset and the full run length.
fn find_run_at_least(input: &str, n: usize) -> Option<(usize, usize)> {
    find_apostrophe_run(input, |run| run >= n)
}

/// Locate the first unescaped run that can close italic: exactly two
/// apostrophes, or the leading two of a five-or-more run (the remainder can
/// still open bold).
fn find_italic_closer(input: &str) -> Option<(usize, usize)> {
    find_apostrophe_run(input, |run| run == 2 || run >= 5)
}

/// Locate the first unescaped apostrophe run accepted by `accept`, returning
/// the offset and the full run length. Runs the predicate rejects are skipped
/// whole so a longer run never closes a shorter one.
fn find_apostrophe_run(input: &str, accept: impl Fn(usize) -> bool) -> Option<(usize, usize)> {
    scan_unescaped(input, |pos, rest| {
        let run = apostrophe_run(rest);
        if run > 0 {
            if accept(run) {
                ScanAction::Stop((pos, run))
            } else {
                ScanAction::Skip(run)
            }
        } else {
            ScanAction::Continue
        }
    })
}

/// Whether an emphasis body is free of constructs that could swallow a
/// virtually appended closer: escapes, brackets, and braces.
fn simple_emphasis_body(body: &str) -> bool {
    !body
        .bytes()
        .any(|b| matches!(b, b'\\' | b'[' | b']' | b'{' | b'}'))
}

/// Whether a body contains an odd number of emphasis openers of `width`; a
/// three-apostrophe width also counts a four-apostrophe run, which opens bold
/// and leaves one literal apostrophe.
fn odd_emphasis_runs(body: &str, width: usize) -> bool {
    let mut parity = false;
    scan_unescaped::<()>(body, |_, rest| {
        let run = apostrophe_run(rest);
        if run > 0 {
            if run == width || (width == 3 && run == 4) {
                parity = !parity;
            }
            ScanAction::Skip(run)
        } else {
            ScanAction::Continue
        }
    });
    parity
}
