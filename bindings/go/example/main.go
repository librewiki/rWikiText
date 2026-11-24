package main

import (
	"fmt"
	"log"

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
	fmt.Println(result.Content)
	fmt.Println("Categories", result.Metadata.Categories)
}
