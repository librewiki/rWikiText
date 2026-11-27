/// Control flow returned by callbacks passed to [`scan_unescaped`].
pub(super) enum ScanAction<T> {
    Continue,
    Skip(usize),
    Stop(T),
}

/// Walk `input` visiting each unescaped UTF-8 position, skipping `\` and its
/// immediately following character (if any). The callback chooses whether to
/// advance by one character, skip a matched byte span, or stop early with a
/// value.
pub(super) fn scan_unescaped<T>(
    input: &str,
    mut visit: impl FnMut(usize, &str) -> ScanAction<T>,
) -> Option<T> {
    let mut pos = 0;
    while pos < input.len() {
        let rest = &input[pos..];
        if let Some(stripped) = rest.strip_prefix('\\') {
            pos += 1;
            if let Some(c) = stripped.chars().next() {
                pos += c.len_utf8();
            }
            continue;
        }
        match visit(pos, rest) {
            ScanAction::Continue => pos += rest.chars().next().unwrap().len_utf8(),
            ScanAction::Skip(bytes) => pos += bytes,
            ScanAction::Stop(value) => return Some(value),
        }
    }
    None
}

pub(super) fn find_unescaped(input: &str, needle: &str) -> Option<usize> {
    scan_unescaped(input, |pos, rest| {
        if rest.starts_with(needle) {
            ScanAction::Stop(pos)
        } else {
            ScanAction::Continue
        }
    })
}

pub(super) fn unescape(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            } else {
                out.push(c);
            }
        } else {
            out.push(c);
        }
    }
    out
}
