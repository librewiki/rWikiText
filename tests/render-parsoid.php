<?php
declare(strict_types=1);

use Wikimedia\Bcp47Code\Bcp47CodeValue;
use Wikimedia\Parsoid\Mocks\MockDataAccess;
use Wikimedia\Parsoid\Mocks\MockPageConfig;
use Wikimedia\Parsoid\Mocks\MockPageContent;
use Wikimedia\Parsoid\Mocks\MockSiteConfig;
use Wikimedia\Parsoid\Parsoid;
use Wikimedia\Parsoid\Utils\Title;

[, $release, $pageTitle, $language, $referenceTime] = $argv;
require $release . '/vendor/autoload.php';

$site = new class ([]) extends MockSiteConfig {
    public function getMagicWordMatcher(string $id): string {
        return $id === 'redirect' ? '/^#REDIRECT$/i' : parent::getMagicWordMatcher($id);
    }
};
$site->setFakeTimestamp((new DateTimeImmutable($referenceTime))->getTimestamp());
$title = Title::newFromText($pageTitle, $site);
$content = new MockPageContent(['main' => stream_get_contents(STDIN)], $title);
$page = new MockPageConfig($site, [
    'pageLanguage' => new Bcp47CodeValue($language),
    'pageLanguageDir' => 'ltr',
], $content);

$parsoid = new Parsoid($site, new MockDataAccess($site, []));
echo $parsoid->wikitext2html($page, [
    'body_only' => true,
    'wrapSections' => false,
    'outputContentVersion' => '2.8.0',
]);
