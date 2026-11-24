package rwikitext_test

import (
	"errors"
	"fmt"
	"reflect"
	"strings"
	"testing"
	"time"

	"github.com/librewiki/rWikiText/bindings/go"
)

func TestConvertFormats(t *testing.T) {
	tests := []struct {
		name           string
		source, target rwikitext.Format
		input, want    string
	}{
		{"wikitext to GFM", rwikitext.Wikitext, rwikitext.GFM, "'''bold'''", "**bold**"},
		{"wikitext to HTML", rwikitext.Wikitext, rwikitext.HTML, "'''bold'''", "<b>bold</b>"},
		{"GFM to wikitext", rwikitext.GFM, rwikitext.Wikitext, "**bold**", "'''bold'''"},
		{"HTML to wikitext", rwikitext.HTML, rwikitext.Wikitext, "<b>bold</b>", "'''bold'''"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			result, err := rwikitext.Convert(test.source, test.target, test.input, rwikitext.Options{})
			if err != nil {
				t.Fatal(err)
			}
			if !strings.Contains(result.Content, test.want) {
				t.Fatalf("content %q does not contain %q", result.Content, test.want)
			}
		})
	}
}

func TestPageMetadataAndStrictMode(t *testing.T) {
	input := "界\n[[Category:Demo|Sort]]"
	result, err := rwikitext.Convert(rwikitext.Wikitext, rwikitext.GFM, input, rwikitext.Options{})
	if err != nil {
		t.Fatal(err)
	}
	if result.Content != "界" || !reflect.DeepEqual(result.Metadata.Categories, []string{"Demo"}) ||
		len(result.Metadata.CategorySortKeys) != 1 || result.Metadata.CategorySortKeys[0] == nil ||
		*result.Metadata.CategorySortKeys[0] != "Sort" {
		t.Fatalf("unexpected result: %+v", result)
	}
	if len(result.Diagnostics) != 1 {
		t.Fatalf("want one diagnostic, got %+v", result.Diagnostics)
	}
	diagnostic := result.Diagnostics[0]
	if diagnostic.Kind != rwikitext.ConversionLoss || diagnostic.Start != 4 ||
		diagnostic.End != len(input) || diagnostic.Message == "" {
		t.Fatalf("unexpected diagnostic: %+v", diagnostic)
	}
	_, err = rwikitext.Convert(rwikitext.Wikitext, rwikitext.GFM, input, rwikitext.Options{Mode: rwikitext.Strict})
	var failure *rwikitext.ConversionError
	if !errors.As(err, &failure) || !reflect.DeepEqual(failure.Diagnostics, result.Diagnostics) {
		t.Fatalf("strict conversion returned %#v", err)
	}

	redirect, err := rwikitext.Convert(rwikitext.Wikitext, rwikitext.HTML,
		"#REDIRECT [[Main Page]]\n{{DISPLAYTITLE:Example}}",
		rwikitext.Options{Context: rwikitext.WikiContext{PageTitle: "Example"}})
	if err != nil {
		t.Fatal(err)
	}
	if redirect.Metadata.RedirectTarget == nil || *redirect.Metadata.RedirectTarget != "Main Page" ||
		redirect.Metadata.DisplayTitle == nil || *redirect.Metadata.DisplayTitle != "Example" {
		t.Fatalf("unexpected redirect metadata: %+v", redirect.Metadata)
	}
}

func TestWikiContextAndExpansionLimits(t *testing.T) {
	input := "Page {{PAGENAME}} language {{CONTENTLANGUAGE}} time {{CURRENTTIMESTAMP}}"
	language := "fr"
	reference := time.Date(2025, 3, 4, 14, 6, 7, 123456789, time.FixedZone("KST", 9*60*60))
	options := rwikitext.Options{Context: rwikitext.WikiContext{
		PageTitle: "Guide", SiteLanguage: &language, ReferenceTime: &reference,
	}}
	result, err := rwikitext.Convert(rwikitext.Wikitext, rwikitext.GFM, input, options)
	if err != nil || result.Content != "Page Guide language fr time 20250304050607" {
		t.Fatalf("configured context returned %q, %v", result.Content, err)
	}
	result, err = rwikitext.Convert(rwikitext.Wikitext, rwikitext.GFM, input, rwikitext.Options{})
	if err != nil || result.Content != "Page  language en time 19700101000000" {
		t.Fatalf("default context returned %q, %v", result.Content, err)
	}

	lookup := func(string) (string, bool, error) { return "expanded", true, nil }
	options = rwikitext.Options{Lookups: rwikitext.Lookups{TemplateText: lookup}}
	result, err = rwikitext.Convert(rwikitext.Wikitext, rwikitext.GFM, "{{One}}", options)
	if err != nil || result.Content != "expanded" {
		t.Fatalf("default limits returned %q, %v", result.Content, err)
	}
	zero := uint(0)
	for _, test := range []struct {
		reason string
		limits rwikitext.ExpansionLimits
	}{
		{"depth", rwikitext.ExpansionLimits{MaxDepth: &zero}},
		{"count", rwikitext.ExpansionLimits{MaxExpansions: &zero}},
		{"output byte", rwikitext.ExpansionLimits{MaxOutputBytes: &zero}},
	} {
		t.Run(test.reason, func(t *testing.T) {
			options.ExpansionLimits = test.limits
			_, err := rwikitext.Convert(rwikitext.Wikitext, rwikitext.GFM, "{{One}}", options)
			checkFailure(t, err, rwikitext.ExpansionLimit, 0, 7, test.reason)
		})
	}
}

func TestWikiDataLookups(t *testing.T) {
	width, height := uint32(300), uint32(200)
	options := rwikitext.Options{Lookups: rwikitext.Lookups{
		TemplateText: func(title string) (string, bool, error) {
			if title != "Template:Épreuve_du_jour" {
				return "", false, fmt.Errorf("unexpected template title %q", title)
			}
			return "expanded", true, nil
		},
		PageExists: func(title string) (bool, bool, error) {
			switch title {
			case "Absent_page":
				return false, true, nil
			case "Existing_page":
				return true, true, nil
			default:
				return false, false, fmt.Errorf("unexpected page title %q", title)
			}
		},
		FileInfo: func(title string) (rwikitext.FileInfo, bool, error) {
			if title != "File:Example_image.png" {
				return rwikitext.FileInfo{}, false, fmt.Errorf("unexpected file title %q", title)
			}
			return rwikitext.FileInfo{URL: "https://example.org/image.png", Width: &width, Height: &height}, true, nil
		},
	}}
	input := "{{épreuve du jour}} [[Absent page]] [[Existing page]] [[File:Example image.png]]"
	result, err := rwikitext.Convert(rwikitext.Wikitext, rwikitext.HTML, input, options)
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{
		"expanded", "Absent_page?action=edit", `href="./Existing_page"`,
		"https://example.org/image.png", `width="300"`, `height="200"`,
	} {
		if !strings.Contains(result.Content, want) {
			t.Errorf("content does not contain %q: %s", want, result.Content)
		}
	}
}

func TestMissingWikiData(t *testing.T) {
	input := "{{Absent}} [[Unknown]] [[File:Unknown.png]]"
	offline, err := rwikitext.Convert(rwikitext.Wikitext, rwikitext.HTML, input, rwikitext.Options{})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(offline.Content, "Template:Absent") {
		t.Fatalf("expected the missing template in %q", offline.Content)
	}
	options := rwikitext.Options{Lookups: rwikitext.Lookups{
		TemplateText: func(string) (string, bool, error) { return "", false, nil },
		PageExists:   func(string) (bool, bool, error) { return false, false, nil },
		FileInfo:     func(string) (rwikitext.FileInfo, bool, error) { return rwikitext.FileInfo{}, false, nil },
	}}
	reported, err := rwikitext.Convert(rwikitext.Wikitext, rwikitext.HTML, input, options)
	if err != nil || !reflect.DeepEqual(offline, reported) {
		t.Fatalf("explicit missing data returned %+v, %v, want %+v", reported, err, offline)
	}
}

func TestLookupFailuresAreLocated(t *testing.T) {
	t.Setenv("GODEBUG", "panicnil=1")
	for _, test := range []struct {
		name    string
		lookup  func(string) (string, bool, error)
		message string
	}{
		{"offline", func(string) (string, bool, error) { return "", false, errors.New("offline") }, "offline"},
		{"panic", func(string) (string, bool, error) { panic("unexpected error") }, "unexpected error"},
		{"panic nil", func(string) (string, bool, error) { panic(nil) }, "lookup panic"},
	} {
		t.Run(test.name, func(t *testing.T) {
			options := rwikitext.Options{Lookups: rwikitext.Lookups{TemplateText: test.lookup}}
			_, err := rwikitext.Convert(rwikitext.Wikitext, rwikitext.HTML, "界 {{Fail}}", options)
			checkFailure(t, err, rwikitext.WikiLookup, 4, 12, test.message)
		})
	}
}

func TestBindingErrorIsDistinctFromConversionError(t *testing.T) {
	_, err := rwikitext.Convert(rwikitext.Wikitext, rwikitext.HTML, "\xff", rwikitext.Options{})
	var native *rwikitext.NativeError
	if !errors.As(err, &native) {
		t.Fatalf("want NativeError for malformed UTF-8, got %#v", err)
	}
	_, err = rwikitext.Convert(rwikitext.HTML, rwikitext.GFM, "<p>test</p>", rwikitext.Options{})
	var conversion *rwikitext.ConversionError
	if !errors.As(err, &conversion) || len(conversion.Diagnostics) != 0 {
		t.Fatalf("want ConversionError for unsupported format pair, got %#v", err)
	}
}

func TestOverlappingAndNestedConversions(t *testing.T) {
	entered := make(chan string, 2)
	release := make(chan struct{})
	type outcome struct {
		result rwikitext.Result
		err    error
	}
	results := make(chan outcome, 2)
	start := func(label string) {
		go func() {
			options := rwikitext.Options{Lookups: rwikitext.Lookups{
				TemplateText: func(title string) (string, bool, error) {
					entered <- title
					<-release
					innerOptions := rwikitext.Options{Lookups: rwikitext.Lookups{
						TemplateText: func(title string) (string, bool, error) {
							if title != "Template:Inner" {
								return "", false, fmt.Errorf("unexpected nested title %q", title)
							}
							return label, true, nil
						},
					}}
					inner, err := rwikitext.Convert(rwikitext.Wikitext, rwikitext.GFM, "{{Inner}}", innerOptions)
					return inner.Content, true, err
				},
			}}
			result, err := rwikitext.Convert(rwikitext.Wikitext, rwikitext.GFM, "{{Outer}}", options)
			results <- outcome{result, err}
		}()
	}
	start("alpha")
	start("beta")
	timer := time.NewTimer(10 * time.Second)
	defer timer.Stop()
	for i := 0; i < 2; i++ {
		select {
		case title := <-entered:
			if title != "Template:Outer" {
				t.Errorf("unexpected outer title %q", title)
			}
		case <-timer.C:
			close(release)
			t.Fatal("conversions did not overlap inside the callback")
		}
	}
	close(release)
	content := make(map[string]int)
	for i := 0; i < 2; i++ {
		select {
		case outcome := <-results:
			if outcome.err != nil {
				t.Fatal(outcome.err)
			}
			content[outcome.result.Content]++
		case <-timer.C:
			t.Fatal("nested conversions did not complete")
		}
	}
	if !reflect.DeepEqual(content, map[string]int{"alpha": 1, "beta": 1}) {
		t.Fatalf("overlapping conversions returned %v", content)
	}
}

func checkFailure(t *testing.T, err error, kind rwikitext.DiagnosticKind, start, end int, message string) {
	t.Helper()
	var failure *rwikitext.ConversionError
	if !errors.As(err, &failure) || len(failure.Diagnostics) != 1 {
		t.Fatalf("want conversion error with one diagnostic, got %#v", err)
	}
	diagnostic := failure.Diagnostics[0]
	if diagnostic.Kind != kind || diagnostic.Start != start || diagnostic.End != end ||
		!strings.Contains(diagnostic.Message, message) {
		t.Fatalf("unexpected diagnostic: %+v", diagnostic)
	}
}
