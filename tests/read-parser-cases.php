<?php

use Wikimedia\Parsoid\ParserTests\Grammar;

$release = $argv[1];
require $release . '/vendor/autoload.php';

$seen = [];
foreach ([
    'tests/parser',
    'vendor/wikimedia/parsoid/tests/parser',
    'extensions/ParserFunctions/tests/parser',
] as $directory) {
    foreach (glob("$release/$directory/*.txt") as $file) {
        $digest = hash_file('sha256', $file);
        if (isset($seen[$digest])) {
            continue;
        }
        $seen[$digest] = true;
        [, , , $options, $fixtures] = Grammar::load($file);
        $cases = array_values(array_filter($fixtures, static fn ($fixture) => $fixture['type'] === 'test'));
        echo json_encode([
            'path' => "$directory/" . basename($file),
            'options' => $options['text'] ?? [],
            'cases' => $cases,
        ], JSON_THROW_ON_ERROR), "\n";
    }
}
