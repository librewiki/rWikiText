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
    val names = listOf("namumark", "wikitext", "gfm", "html")
    val inputs = listOf("== 제목 ==\\n'''안녕'''", "== 제목 ==\\n'''안녕'''", "## 제목\\n\\n**안녕**", "<h2>제목</h2><p><strong>안녕</strong></p>")
    val markers = listOf(listOf("==", "'''"), listOf("==", "'''"), listOf("##", "**"), listOf("<h2", "<b"))
    for (other in 1..3) for ((source, target) in listOf(0 to other, other to 0)) {
        val converted = convert("""{"source":"${names[source]}","target":"${names[target]}","input":"${inputs[source]}"}""")
        check("\"ok\":true" in converted && "제목" in converted && "안녕" in converted && markers[target].all { it in converted }) { converted }
    }
    var namuCalls = 0
    val unexpected = Lookup { _, _ -> namuCalls++; Reply.Missing }
    val category = convert("""{"source":"namumark","target":"gfm","input":"[[분류:Example|Sort]]"}""", unexpected)
    check("\"ok\":true" in category && "\"categories\":[\"Example\"]" in category && "\"category_sort_keys\":[\"Sort\"]" in category && "conversion_loss" in category) { category }
    val strict = convert("""{"source":"namumark","target":"html","input":"||cell||","mode":"strict"}""", unexpected)
    check("\"ok\":false" in strict && "conversion_loss" in strict) { strict }
    val literal = convert("""{"source":"namumark","target":"html","input":"{{{literal}}}"}""", unexpected)
    check("\"ok\":true" in literal && namuCalls == 0) { literal }
    println(found)
}
