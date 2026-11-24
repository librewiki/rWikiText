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

// Optional arguments declare pages that exist in this fixture's wiki context.
// Without them, every lookup retains MockDataAccess's original behavior.
$data = new class ($site, [], array_slice($argv, 5)) extends MockDataAccess {
    private array $fixturePageIds;

    public function __construct($site, array $opts, array $knownFixtureTitles) {
        parent::__construct($site, $opts);
        $this->fixturePageIds = [];
        foreach ($knownFixtureTitles as $index => $title) {
            $this->fixturePageIds[$title] = 1000000 + $index;
        }
    }

    public function getPageInfo($pageConfigOrTitle, array $titles, bool $defaultLinkCaption = false): array {
        $result = parent::getPageInfo($pageConfigOrTitle, $titles, $defaultLinkCaption);
        foreach ($result as $title => &$info) {
            if (isset($this->fixturePageIds[$title])) {
                $info['known'] = true;
                $info['missing'] = false;
                $info['pageId'] = $this->fixturePageIds[$title];
            }
        }
        unset($info);
        return $result;
    }
};
$parsoid = new Parsoid($site, $data);
echo $parsoid->wikitext2html($page, [
    'body_only' => true,
    'wrapSections' => false,
    'outputContentVersion' => '2.8.0',
]);
