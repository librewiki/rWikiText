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
