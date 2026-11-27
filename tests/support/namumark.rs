use rwikitext::{
    convert, convert_with_mode, ConversionMode, ConversionRequest, ConversionResult, Format,
    WikiContext,
};

/// Convert NamuMark `input` to `target` under `context`, asserting no
/// diagnostics in normal mode and identical output in strict mode.
pub fn convert_clean_namumark(
    context: &WikiContext<'_>,
    target: Format,
    input: &str,
) -> ConversionResult {
    let request = ConversionRequest {
        source: Format::NamuMark,
        target,
        input,
        context,
    };
    let result = convert(request).unwrap();
    assert!(
        result.diagnostics.is_empty(),
        "target {target:?} input {input:?}: {:?}",
        result.diagnostics
    );
    let strict = convert_with_mode(request, ConversionMode::Strict).unwrap_or_else(|err| {
        panic!("strict mode failed for target {target:?} input {input:?}: {err:?}")
    });
    assert_eq!(
        strict.content, result.content,
        "strict content mismatch for target {target:?} input {input:?}"
    );
    result
}

/// Assert that converting NamuMark `input` to `target` under `context` produces
/// `expected` with no diagnostics in normal mode and succeeds with identical
/// content in strict mode.
pub fn assert_clean_namumark(
    context: &WikiContext<'_>,
    target: Format,
    input: &str,
    expected: &str,
) {
    let result = convert_clean_namumark(context, target, input);
    assert_eq!(result.content, expected, "target {target:?} input {input:?}");
}
