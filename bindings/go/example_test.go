package rwikitext_test

import (
	"errors"
	"fmt"
	"log"

	"github.com/librewiki/rWikiText/bindings/go"
)

func ExampleConvert() {
	result, err := rwikitext.Convert(
		rwikitext.Wikitext,
		rwikitext.GFM,
		"'''Bold''' text",
		rwikitext.Options{},
	)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(result.Content)
	// Output: **Bold** text
}

func ExampleConvert_lookups() {
	options := rwikitext.Options{
		Context: rwikitext.WikiContext{PageTitle: "Guide"},
		Lookups: rwikitext.Lookups{
			TemplateText: func(title string) (string, bool, error) {
				if title == "Template:Greeting" {
					return "Hello, {{{1}}}!", true, nil
				}
				return "", false, nil
			},
		},
	}
	result, err := rwikitext.Convert(
		rwikitext.Wikitext,
		rwikitext.GFM,
		"{{Greeting|{{PAGENAME}}}}\n[[Category:Docs|Guide]]",
		options,
	)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(result.Metadata.Categories)
	// Output: [Docs]
}

func ExampleConvert_strictMode() {
	_, err := rwikitext.Convert(
		rwikitext.Wikitext,
		rwikitext.GFM,
		"[[Category:Demo]]",
		rwikitext.Options{Mode: rwikitext.Strict},
	)
	var convErr *rwikitext.ConversionError
	if errors.As(err, &convErr) {
		fmt.Println(convErr.Diagnostics[0].Kind)
	}
	// Output: conversion_loss
}
