use std::ops::Range;

pub(crate) fn range_after_removals(span: Range<usize>, ranges: &[Range<usize>]) -> Range<usize> {
    let boundary = |offset: usize| {
        offset
            - ranges
                .iter()
                .map(|range| offset.min(range.end).saturating_sub(range.start))
                .sum::<usize>()
    };
    boundary(span.start)..boundary(span.end)
}

pub(crate) fn original_range(span: Range<usize>, removed: &[Range<usize>]) -> Range<usize> {
    let mut start = span.start;
    let mut end = span.end;
    for removal in removed {
        if removal.start <= start {
            start += removal.len();
        }
        if removal.start < end {
            end += removal.len();
        }
    }
    start..end.max(start)
}

pub(crate) fn locate(input: &str, fragment: &str) -> Range<usize> {
    input
        .find(fragment)
        .map_or(0..input.len(), |start| start..start + fragment.len())
}
