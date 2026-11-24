package main

import (
	"errors"
	"fmt"
	"log"
	"strings"

	"github.com/librewiki/rWikiText/bindings/go"
)

func main() {
	input := `{{Greeting|{{PAGENAME}}}} [[Absent]] [[File:Example.png|alt=Example]]
[[Category:Demo|Sort]]`
	options := rwikitext.Options{
		Context: rwikitext.WikiContext{PageTitle: "Guide"},
		Lookups: rwikitext.Lookups{
			TemplateText: func(title string) (string, bool, error) {
				if title == "Template:Greeting" {
					return "Hello, {{{1}}}!", true, nil
				}
				return "", false, nil
			},
			PageExists: func(title string) (bool, bool, error) {
				if title == "Absent" {
					return false, true, nil
				}
				return false, false, nil
			},
			FileInfo: func(title string) (rwikitext.FileInfo, bool, error) {
				if title == "File:Example.png" {
					return rwikitext.FileInfo{URL: "https://example.org/image.png"}, true, nil
				}
				return rwikitext.FileInfo{}, false, nil
			},
		},
	}
	result, err := rwikitext.Convert(rwikitext.Wikitext, rwikitext.HTML, input, options)
	if err != nil {
		log.Fatal(err)
	}
	inputs := map[rwikitext.Format]string{rwikitext.NamuMark: "== 제목 ==\n'''안녕'''", rwikitext.Wikitext: "== 제목 ==\n'''안녕'''", rwikitext.GFM: "## 제목\n\n**안녕**", rwikitext.HTML: "<h2>제목</h2><p><strong>안녕</strong></p>"}
	markers := map[rwikitext.Format][]string{rwikitext.NamuMark: {"==", "'''"}, rwikitext.Wikitext: {"==", "'''"}, rwikitext.GFM: {"##", "**"}, rwikitext.HTML: {"<h2", "<b"}}
	for _, other := range []rwikitext.Format{rwikitext.Wikitext, rwikitext.GFM, rwikitext.HTML} {
		for _, pair := range [][2]rwikitext.Format{{rwikitext.NamuMark, other}, {other, rwikitext.NamuMark}} {
			converted, err := rwikitext.Convert(pair[0], pair[1], inputs[pair[0]], rwikitext.Options{})
			if err != nil {
				log.Fatal(err)
			}
			for _, part := range append([]string{"제목", "안녕"}, markers[pair[1]]...) {
				if !strings.Contains(converted.Content, part) {
					log.Fatalf("%s -> %s: %s", pair[0], pair[1], converted.Content)
				}
			}
		}
	}
	namuCalls := 0
	namuOptions := rwikitext.Options{Lookups: rwikitext.Lookups{TemplateText: func(string) (string, bool, error) { namuCalls++; return "", false, nil }}}
	category, err := rwikitext.Convert(rwikitext.NamuMark, rwikitext.GFM, "[[분류:Example|Sort]]", namuOptions)
	if err != nil || len(category.Metadata.Categories) != 1 || category.Metadata.Categories[0] != "Example" || len(category.Metadata.CategorySortKeys) != 1 || category.Metadata.CategorySortKeys[0] == nil || *category.Metadata.CategorySortKeys[0] != "Sort" || len(category.Diagnostics) == 0 || category.Diagnostics[0].Kind != rwikitext.ConversionLoss {
		log.Fatalf("category: %+v %v", category, err)
	}
	namuOptions.Mode = rwikitext.Strict
	_, err = rwikitext.Convert(rwikitext.NamuMark, rwikitext.HTML, "||cell||", namuOptions)
	var loss *rwikitext.ConversionError
	if !errors.As(err, &loss) || len(loss.Diagnostics) == 0 || loss.Diagnostics[0].Kind != rwikitext.ConversionLoss {
		log.Fatalf("strict loss: %v", err)
	}
	namuOptions.Mode = rwikitext.BestEffort
	_, err = rwikitext.Convert(rwikitext.NamuMark, rwikitext.HTML, "{{{literal}}}", namuOptions)
	if err != nil || namuCalls != 0 {
		log.Fatalf("NamuMark lookup: %v calls=%d", err, namuCalls)
	}
	fmt.Println(result.Content)
	fmt.Println("Categories", result.Metadata.Categories)
}
