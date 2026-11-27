<?php
declare(strict_types=1);

function check(bool $condition, string $message): void {
    if (!$condition) { throw new RuntimeException($message); }
}

$calls = [];
$lookup = static function (string $operation, string $title) use (&$calls) {
    $calls[] = [$operation, $title];
    return match ($operation) {
        'template_text' => $title === 'Template:Hello' ? 'Hello {{{1}}}' : null,
        'page_exists' => $title === 'Present',
        'file_info' => ['url' => 'https://example.org/image.png', 'width' => 300, 'height' => 200],
    };
};
$request = [
    'source' => 'wikitext', 'target' => 'html',
    'input' => '{{Hello|world}} {{PAGENAME}} {{#ifexist:Present|found|missing}} [[File:Example.png]] [[Category:Demo|Sort]]',
    'context' => ['page_title' => 'Example', 'site_language' => 'en', 'reference_time' => '2024-02-03T04:05:06Z'],
    'mode' => 'best_effort',
    'expansion_limits' => ['max_depth' => 20, 'max_expansions' => 100, 'max_output_bytes' => 65536],
];
$result = rwikitext_convert($request, $lookup);
check($result['ok'] === true, 'conversion failed: ' . json_encode($result));
check(str_contains($result['content'], 'Hello world'), 'template expansion failed');
check(str_contains($result['content'], 'Example'), 'context was not applied');
check(str_contains($result['content'], 'found'), 'page lookup failed');
check(str_contains($result['content'], 'https://example.org/image.png'), 'file lookup failed');
check($result['metadata']['categories'] === ['Demo'], 'category metadata missing');
check($result['metadata']['category_sort_keys'] === ['Sort'], 'category sort key missing');
check(is_array($result['diagnostics']), 'diagnostics missing');
foreach (['template_text', 'page_exists', 'file_info'] as $operation) {
    check(in_array($operation, array_column($calls, 0), true), "lookup not called: $operation");
}
$missing = rwikitext_convert(['source' => 'wikitext', 'target' => 'html', 'input' => '{{#ifexist:Absent|yes|no}}'], static fn() => null);
check($missing['ok'] && str_contains($missing['content'], 'no'), 'missing lookup failed');
$failure = rwikitext_convert(['source' => 'wikitext', 'target' => 'html', 'input' => '{{Fail}}'], static function () { throw new RuntimeException("lookup offline\0details"); });
check(!$failure['ok'], 'lookup exception did not fail conversion');
check(str_contains($failure['error'], "lookup offline\0details"), 'lookup error message lost');
check(in_array('wiki_lookup', array_column($failure['diagnostics'], 'kind'), true), 'lookup diagnostic missing');
$limit = rwikitext_convert(['source' => 'wikitext', 'target' => 'html', 'input' => '{{Hello|world}}', 'expansion_limits' => ['max_expansions' => 0]], $lookup);
check(!$limit['ok'] && in_array('expansion_limit', array_column($limit['diagnostics'], 'kind'), true), 'expansion limit was not enforced');
echo "rWikiText PHP example passed\n";

$inputs = ['namumark' => "== 제목 ==\n'''안녕'''", 'wikitext' => "== 제목 ==\n'''안녕'''", 'gfm' => "## 제목\n\n**안녕**", 'html' => '<h2>제목</h2><p><strong>안녕</strong></p>'];
$markers = ['namumark' => ['==', "'''"], 'wikitext' => ['==', "'''"], 'gfm' => ['##', '**'], 'html' => ['<h2', '<strong']];
foreach (['wikitext', 'gfm', 'html'] as $other) foreach ([['namumark', $other], [$other, 'namumark']] as [$source, $target]) {
    $converted = rwikitext_convert(['source' => $source, 'target' => $target, 'input' => $inputs[$source]]);
    check($converted['ok'], json_encode($converted));
    foreach (array_merge(['제목', '안녕'], $markers[$target]) as $part) check(str_contains($converted['content'], $part), json_encode($converted));
}

$namuCalls = 0;
$unexpectedLookup = static function () use (&$namuCalls) { $namuCalls++; return null; };
$category = rwikitext_convert(['source' => 'namumark', 'target' => 'gfm', 'input' => '[[분류:Example|Sort]]'], $unexpectedLookup);
check($category['ok'] && $category['metadata']['categories'] === ['Example'] && $category['metadata']['category_sort_keys'] === ['Sort'], 'NamuMark category');
check(in_array('conversion_loss', array_column($category['diagnostics'], 'kind'), true), 'NamuMark category diagnostic');
$strict = rwikitext_convert(['source' => 'namumark', 'target' => 'html', 'input' => '||cell||', 'mode' => 'strict'], $unexpectedLookup);
check(!$strict['ok'] && in_array('conversion_loss', array_column($strict['diagnostics'], 'kind'), true), 'NamuMark strict loss');
$literal = rwikitext_convert(['source' => 'namumark', 'target' => 'html', 'input' => '{{{literal}}}'], $unexpectedLookup);
check($literal['ok'] && $namuCalls === 0, 'NamuMark invoked lookup');
