<?php

/**
 * PHP 8 extension stub for the rWikiText native converter (C ABI v1).
 *
 * This file provides PHPDoc API reference, static-analysis array shapes, and
 * usage examples for IDEs, static analyzers (PHPStan/Psalm), humans, and AI tools.
 * It is not loaded at runtime when the compiled `rwikitext` extension is enabled.
 *
 * @generate-class-entries
 */

/**
 * Converts a UTF-8 document between MediaWiki wikitext, NamuMark, GFM, and HTML.
 *
 * Supported format pairs:
 * - `'wikitext'` <-> `'gfm'`, `'html'`, `'namumark'`
 * - `'namumark'` <-> `'wikitext'`, `'gfm'`, `'html'`
 *
 * Identity conversions (`source === target`) and direct `'gfm'` <-> `'html'`
 * conversions are unsupported and return an array with `'ok' => false`.
 *
 * The optional `$lookup` callback runs synchronously on the calling PHP thread as
 * `$lookup(string $operation, string $title)`. Titles use canonical namespace casing
 * and underscore-separated spelling (for example, `'Template:Greeting'`, `'Main_Page'`,
 * or `'File:Logo.png'`). Return `null` to report missing data, or:
 * - `'template_text'` => `string` (raw template wikitext)
 * - `'page_exists'` => `bool` (`false` explicitly marks a page absent)
 * - `'file_info'` => `array{url: string, width?: int, height?: int}`
 *
 * Exceptions thrown by `$lookup` are caught and reported as a `'wiki_lookup'`
 * diagnostic in a failure response (`'ok' => false`). NamuMark input does not
 * invoke `$lookup`.
 *
 * @param array{
 *     source: 'wikitext'|'namumark'|'gfm'|'html',
 *     target: 'wikitext'|'namumark'|'gfm'|'html',
 *     input: string,
 *     context?: array{
 *         page_title?: string,
 *         site_language?: string,
 *         reference_time?: string
 *     },
 *     mode?: 'best_effort'|'strict',
 *     expansion_limits?: array{
 *         max_depth?: int<0, max>,
 *         max_expansions?: int<0, max>,
 *         max_output_bytes?: int<0, max>
 *     }
 * } $request Associative array describing the conversion request.
 * @param (callable(string, string): (string|bool|array{url: string, width?: int, height?: int}|null))|null $lookup
 *     Optional synchronous wiki-data callback, or `null` to report all lookups missing.
 *
 * @return array{
 *     ok: true,
 *     content: string,
 *     metadata: array{
 *         categories: list<string>,
 *         category_sort_keys: list<string|null>,
 *         redirect_target: string|null,
 *         display_title: string|null
 *     },
 *     diagnostics: list<array{
 *         kind: 'conversion_loss'|'html_removal'|'wiki_lookup'|'expansion_limit',
 *         start: int,
 *         end: int,
 *         message: string
 *     }>
 * }|array{
 *     ok: false,
 *     error: string,
 *     diagnostics: list<array{
 *         kind: 'conversion_loss'|'html_removal'|'wiki_lookup'|'expansion_limit',
 *         start: int,
 *         end: int,
 *         message: string
 *     }>
 * } Decoded response envelope containing converted content and metadata on success,
 *   or an error message and diagnostics on failure.
 *
 * @throws \TypeError If `$lookup` is neither callable nor `null`.
 * @throws \Exception If the native ABI version is unsupported, the request cannot
 *                    be encoded as JSON, or the native converter fails internally.
 *
 * @example
 * ```php
 * <?php
 * declare(strict_types=1);
 *
 * $lookup = static function (string $operation, string $title): string|bool|array|null {
 *     return match ($operation) {
 *         'template_text' => $title === 'Template:Greeting' ? 'Hello, {{{1}}}!' : null,
 *         'page_exists' => $title === 'Main_Page',
 *         'file_info' => $title === 'File:Logo.png'
 *             ? ['url' => 'https://example.org/logo.png', 'width' => 200, 'height' => 100]
 *             : null,
 *         default => null,
 *     };
 * };
 *
 * $result = rwikitext_convert([
 *     'source' => 'wikitext',
 *     'target' => 'gfm',
 *     'input' => "{{Greeting|{{PAGENAME}}}}\n[[Category:Docs|Guide]]",
 *     'context' => ['page_title' => 'Guide'],
 *     'mode' => 'best_effort',
 * ], $lookup);
 *
 * if ($result['ok']) {
 *     echo $result['content'], "\n";
 *     print_r($result['metadata']['categories']);
 * } else {
 *     fwrite(STDERR, $result['error'] . "\n");
 * }
 * ```
 */
function rwikitext_convert(array $request, ?callable $lookup = null): array {}
