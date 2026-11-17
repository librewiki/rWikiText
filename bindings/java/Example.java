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
        System.out.println(found);
    }
}
