import org.librewiki.rwikitext.WikiText;

public class Example {
    public static void main(String[] args) {
        String request = "{\"source\":\"wikitext\",\"target\":\"gfm\",\"input\":\"{{Greeting|{{PAGENAME}}}}\\n[[Category:Demo|Sort]]\",\"context\":{\"page_title\":\"Guide\"},\"mode\":\"best_effort\"}";
        int[] calls = {0};
        String found = WikiText.convert(request, (op, title) -> {
            if (op != WikiText.TEMPLATE_TEXT || !title.equals("Template:Greeting")) throw new AssertionError(title);
            calls[0]++;
            return WikiText.Reply.found("\"Hello, {{{1}}}! 안녕 🌍\"");
        });
        if (calls[0] == 0 || !found.contains("\"ok\":true") || !found.contains("Hello") || !found.contains("Guide") || !found.contains("안녕 🌍") || !found.contains("Demo") || !found.contains("Sort") || !found.contains("conversion_loss")) throw new AssertionError(found);
        String unknown = "{\"source\":\"wikitext\",\"target\":\"html\",\"input\":\"{{Unknown}}\"}";
        String missing = WikiText.convert(unknown, (op, title) -> WikiText.Reply.missing());
        if (!missing.contains("\"ok\":true") || !missing.contains("Template:Unknown")) throw new AssertionError(missing);
        String failed = WikiText.convert(request, (op, title) -> WikiText.Reply.error("\"offline\""));
        if (!failed.contains("\"ok\":false") || !failed.contains("offline") || !failed.contains("wiki_lookup")) throw new AssertionError(failed);
        String[] names = {"namumark", "wikitext", "gfm", "html"};
        String[] inputs = {"== 제목 ==\\n'''안녕'''", "== 제목 ==\\n'''안녕'''", "## 제목\\n\\n**안녕**", "<h2>제목</h2><p><strong>안녕</strong></p>"};
        String[][] markers = {{"==", "'''"}, {"==", "'''"}, {"##", "**"}, {"<h2", "<strong"}};
        for (int other = 1; other < names.length; other++) for (int direction = 0; direction < 2; direction++) {
            int source = direction == 0 ? 0 : other, target = direction == 0 ? other : 0;
            String converted = WikiText.convert("{\"source\":\"" + names[source] + "\",\"target\":\"" + names[target] + "\",\"input\":\"" + inputs[source] + "\"}", null);
            if (!converted.contains("\"ok\":true") || !converted.contains("제목") || !converted.contains("안녕") || !converted.contains(markers[target][0]) || !converted.contains(markers[target][1])) throw new AssertionError(converted);
        }
        int[] namuCalls = {0};
        WikiText.Lookup unexpected = (op, title) -> { namuCalls[0]++; return WikiText.Reply.missing(); };
        String category = WikiText.convert("{\"source\":\"namumark\",\"target\":\"gfm\",\"input\":\"[[분류:Example|Sort]]\"}", unexpected);
        if (!category.contains("\"ok\":true") || !category.contains("\"categories\":[\"Example\"]") || !category.contains("\"category_sort_keys\":[\"Sort\"]") || !category.contains("conversion_loss")) throw new AssertionError(category);
        String strict = WikiText.convert("{\"source\":\"namumark\",\"target\":\"html\",\"input\":\"||cell||\",\"mode\":\"strict\"}", unexpected);
        if (!strict.contains("\"ok\":false") || !strict.contains("conversion_loss")) throw new AssertionError(strict);
        String literal = WikiText.convert("{\"source\":\"namumark\",\"target\":\"html\",\"input\":\"{{{literal}}}\"}", unexpected);
        if (!literal.contains("\"ok\":true") || namuCalls[0] != 0) throw new AssertionError(literal);
        System.out.println(found);
    }
}
