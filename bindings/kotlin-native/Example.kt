import org.librewiki.rwikitext.*

fun main() {
    val request = """{"source":"wikitext","target":"gfm","input":"{{Greeting|{{PAGENAME}}}}\n[[Category:Demo|Sort]]","context":{"page_title":"Guide"},"mode":"best_effort"}"""
    var calls = 0
    val found = convert(request, Lookup { op, title ->
        check(op == 1u && title == "Template:Greeting")
        calls++
        Reply.Found("\"Hello, {{{1}}}! 안녕 🌍\"")
    })
    check(calls > 0 && "\"ok\":true" in found && "Hello" in found && "Guide" in found && "안녕 🌍" in found && "Demo" in found && "Sort" in found && "conversion_loss" in found) { found }
    val unknown = """{"source":"wikitext","target":"html","input":"{{Unknown}}"}"""
    val missing = convert(unknown, Lookup { _, _ -> Reply.Missing })
    check("\"ok\":true" in missing && "Template:Unknown" in missing) { missing }
    val failed = convert(request, Lookup { _, _ -> Reply.Error("\"offline\"") })
    check("\"ok\":false" in failed && "offline" in failed && "wiki_lookup" in failed) { failed }
    println(found)
}
