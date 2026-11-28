// Package rwikitext converts UTF-8 documents using the rWikiText version 1 C ABI.
//
// It supports MediaWiki wikitext ([Wikitext]), NamuMark ([NamuMark]),
// GitHub Flavored Markdown ([GFM]), and HTML ([HTML]).
//
// # Supported Format Pairs
//
//   - [Wikitext] <-> [GFM], [HTML], [NamuMark]
//   - [NamuMark] <-> [Wikitext], [GFM], [HTML]
//
// Identity conversions (source == target) and direct [GFM] <-> [HTML]
// conversions are unsupported and return a [*ConversionError].
//
// # Quick Start
//
//	result, err := rwikitext.Convert(
//		rwikitext.Wikitext,
//		rwikitext.GFM,
//		"== Overview ==\n'''Bold''' text on {{PAGENAME}}.",
//		rwikitext.Options{
//			Context: rwikitext.WikiContext{PageTitle: "Guide"},
//		},
//	)
//	if err != nil {
//		log.Fatal(err)
//	}
//	fmt.Println(result.Content)
package rwikitext

/*
#cgo CFLAGS: -I${SRCDIR}/../../include
#cgo linux LDFLAGS: ${SRCDIR}/../../target/release/librwikitext.a -ldl -lpthread -lm
#include <stdlib.h>
#include <stdint.h>
#include "rwikitext.h"
const char *rwikitext_go_convert(const char *request, uintptr_t handle);
*/
import "C"

import (
	"encoding/json"
	"fmt"
	"runtime/cgo"
	"time"
	"unicode/utf8"
	"unsafe"
)

// Format identifies a supported input or output document format.
type Format string

const (
	// Wikitext selects MediaWiki wikitext.
	Wikitext Format = "wikitext"
	// NamuMark selects NamuMark wiki markup.
	NamuMark Format = "namumark"
	// GFM selects GitHub Flavored Markdown.
	GFM Format = "gfm"
	// HTML selects HTML (Parsoid-compatible when converting from MediaWiki wikitext).
	HTML Format = "html"
)

// Mode controls how conversion loss is handled. The zero value defaults to [BestEffort].
type Mode string

const (
	// BestEffort completes the conversion and records lossy constructs in [Result.Diagnostics].
	BestEffort Mode = "best_effort"
	// Strict fails with [*ConversionError] if any construct would lose document meaning.
	Strict Mode = "strict"
)

// WikiContext holds page and site settings for a conversion.
//
// MediaWiki magic words such as {{PAGENAME}}, {{CONTENTLANGUAGE}}, and
// {{CURRENTTIMESTAMP}} read from these fields. Zero or nil fields use the native
// defaults: empty page title, site language "en", and reference time 1970-01-01T00:00:00Z.
type WikiContext struct {
	// PageTitle is the current page title (e.g. "Guide").
	PageTitle string `json:"page_title,omitempty"`
	// SiteLanguage is the site language code (e.g. "en", "ko").
	SiteLanguage *string `json:"site_language,omitempty"`
	// ReferenceTime is converted to UTC at whole-second precision.
	ReferenceTime *time.Time `json:"reference_time,omitempty"`
}

// ExpansionLimits bounds MediaWiki template and parser-function expansion.
//
// Nil fields use the native defaults: MaxDepth 40, MaxExpansions 10000, and
// MaxOutputBytes 2097152 (2 MiB).
type ExpansionLimits struct {
	// MaxDepth limits template nesting depth.
	MaxDepth *uint `json:"max_depth,omitempty"`
	// MaxExpansions limits the number of template and parser-function expansions.
	MaxExpansions *uint `json:"max_expansions,omitempty"`
	// MaxOutputBytes limits the expanded wikitext size in bytes.
	MaxOutputBytes *uint `json:"max_output_bytes,omitempty"`
}

// FileInfo holds the URL and optional pixel dimensions returned by [Lookups.FileInfo].
type FileInfo struct {
	// URL is the absolute or root-relative file URL.
	URL string `json:"url"`
	// Width is the image width in pixels, if known.
	Width *uint32 `json:"width,omitempty"`
	// Height is the image height in pixels, if known.
	Height *uint32 `json:"height,omitempty"`
}

// Lookups provides synchronous wiki-data callbacks run on the calling goroutine
// during MediaWiki conversion.
//
// Each callback receives a canonical namespace-prefixed, underscore-separated title
// (e.g. "Template:Greeting", "Main_Page", "File:Example.png").
// Returning (zero, false, nil) or leaving a callback nil reports missing data.
// Returning an error or panicking aborts conversion with a [WikiLookup] diagnostic.
// NamuMark input never calls these callbacks.
type Lookups struct {
	// TemplateText returns raw wikitext for a transcluded template.
	TemplateText func(title string) (text string, found bool, err error)
	// PageExists reports whether a page exists. Returning (false, true, nil)
	// marks the page absent (red link in HTML).
	PageExists func(title string) (exists bool, found bool, err error)
	// FileInfo returns URL and optional dimensions for a wiki file.
	FileInfo func(title string) (info FileInfo, found bool, err error)
}

// Options configures [Convert]. Its zero value uses native defaults
// ([BestEffort], default [WikiContext], default [ExpansionLimits], no lookups).
type Options struct {
	// Context sets page title, site language, and reference time.
	Context WikiContext
	// Mode selects [BestEffort] or [Strict].
	Mode Mode
	// ExpansionLimits bounds MediaWiki template expansion.
	ExpansionLimits ExpansionLimits
	// Lookups supplies optional wiki-data callbacks.
	Lookups Lookups
}

// DiagnosticKind identifies the category of a [Diagnostic].
type DiagnosticKind string

const (
	// ConversionLoss means a source construct could not be represented losslessly.
	ConversionLoss DiagnosticKind = "conversion_loss"
	// HTMLRemoval means disallowed raw HTML was stripped.
	HTMLRemoval DiagnosticKind = "html_removal"
	// WikiLookup means a [Lookups] callback errored, panicked, or returned an invalid reply.
	WikiLookup DiagnosticKind = "wiki_lookup"
	// ExpansionLimit means MediaWiki expansion exceeded [ExpansionLimits].
	ExpansionLimit DiagnosticKind = "expansion_limit"
)

// Diagnostic is a warning or error tied to a half-open UTF-8 byte range [Start, End) in the input.
type Diagnostic struct {
	// Kind is the diagnostic category.
	Kind DiagnosticKind `json:"kind"`
	// Message describes the issue.
	Message string `json:"message"`
	// Start is the inclusive UTF-8 byte offset in the input.
	Start int `json:"start"`
	// End is the exclusive UTF-8 byte offset in the input.
	End int `json:"end"`
}

// PageMetadata holds page metadata extracted outside the rendered body.
type PageMetadata struct {
	// Categories lists category names in document order.
	Categories []string `json:"categories"`
	// CategorySortKeys is parallel to Categories; nil means no explicit sort key.
	CategorySortKeys []*string `json:"category_sort_keys"`
	// RedirectTarget is the redirect destination page title, or nil if not a redirect.
	RedirectTarget *string `json:"redirect_target"`
	// DisplayTitle is the custom display title from {{DISPLAYTITLE:...}}, or nil if unset.
	DisplayTitle *string `json:"display_title"`
}

// Result holds the converted text, extracted metadata, and non-fatal diagnostics.
type Result struct {
	// Content is the converted document in the target format.
	Content string `json:"content"`
	// Metadata holds page metadata extracted from the source.
	Metadata PageMetadata `json:"metadata"`
	// Diagnostics lists non-fatal warnings emitted during conversion.
	Diagnostics []Diagnostic `json:"diagnostics"`
}

// ConversionError is returned when the converter rejects or aborts a conversion
// (unsupported format pair, [Strict] loss, [WikiLookup] failure, or exceeded [ExpansionLimits]).
type ConversionError struct {
	// Message describes why conversion failed.
	Message string
	// Diagnostics lists diagnostics associated with the failure, if any.
	Diagnostics []Diagnostic
}

// Error implements the error interface.
func (e *ConversionError) Error() string { return e.Message }

// NativeError is returned for invalid UTF-8 input, an ABI version mismatch,
// or JSON request/response marshaling failures.
type NativeError struct {
	// Message describes the binding or ABI failure.
	Message string
}

// Error implements the error interface.
func (e *NativeError) Error() string { return e.Message }

// Convert converts UTF-8 input from source to target using options.
//
// Safe for concurrent calls across goroutines and nested calls from inside [Lookups].
// Errors are either [*ConversionError] or [*NativeError].
func Convert(source, target Format, input string, options Options) (Result, error) {
	if !utf8.ValidString(input) {
		return Result{}, &NativeError{Message: "conversion input must be valid UTF-8"}
	}
	if C.rwikitext_v1_abi_version() != C.RWIKITEXT_ABI_VERSION {
		return Result{}, &NativeError{Message: "unsupported rWikiText ABI version"}
	}
	if options.Context.ReferenceTime != nil {
		reference := options.Context.ReferenceTime.UTC().Truncate(time.Second)
		options.Context.ReferenceTime = &reference
	}
	request := struct {
		Source  Format          `json:"source"`
		Target  Format          `json:"target"`
		Input   string          `json:"input"`
		Context WikiContext     `json:"context"`
		Mode    Mode            `json:"mode,omitempty"`
		Limits  ExpansionLimits `json:"expansion_limits"`
	}{source, target, input, options.Context, options.Mode, options.ExpansionLimits}
	data, err := json.Marshal(request)
	if err != nil {
		return Result{}, &NativeError{Message: fmt.Sprintf("encode request: %v", err)}
	}
	requestC := C.CString(string(data))
	defer C.free(unsafe.Pointer(requestC))
	var handle cgo.Handle
	if options.Lookups.TemplateText != nil || options.Lookups.PageExists != nil || options.Lookups.FileInfo != nil {
		handle = cgo.NewHandle(options.Lookups)
		defer handle.Delete()
	}
	response := C.rwikitext_go_convert(requestC, C.uintptr_t(handle))
	if response == nil {
		return Result{}, &NativeError{Message: "rWikiText returned a null response"}
	}
	defer C.rwikitext_v1_free(response)
	var decoded struct {
		OK          *bool         `json:"ok"`
		Error       *string       `json:"error"`
		Content     *string       `json:"content"`
		Metadata    *PageMetadata `json:"metadata"`
		Diagnostics []Diagnostic  `json:"diagnostics"`
	}
	if err := json.Unmarshal([]byte(C.GoString(response)), &decoded); err != nil {
		return Result{}, &NativeError{Message: fmt.Sprintf("decode response: %v", err)}
	}
	if decoded.OK == nil || decoded.Diagnostics == nil {
		return Result{}, &NativeError{Message: "invalid rWikiText response envelope"}
	}
	if !*decoded.OK {
		if decoded.Error == nil {
			return Result{}, &NativeError{Message: "missing rWikiText error message"}
		}
		return Result{}, &ConversionError{Message: *decoded.Error, Diagnostics: decoded.Diagnostics}
	}
	if decoded.Content == nil || decoded.Metadata == nil {
		return Result{}, &NativeError{Message: "missing rWikiText result"}
	}
	return Result{Content: *decoded.Content, Metadata: *decoded.Metadata, Diagnostics: decoded.Diagnostics}, nil
}

//export rwikitextGoLookup
func rwikitextGoLookup(handle C.uintptr_t, operation C.uint32_t, title *C.char, titleLen C.size_t, reply *C.rwikitext_v1_reply) {
	// Also catch panic(nil) when the caller enables legacy panic behavior.
	returned := false
	defer func() {
		if value := recover(); !returned {
			setReply(reply, C.RWIKITEXT_V1_ERROR, fmt.Sprintf("lookup panic: %v", value))
		}
	}()
	lookup(handle, operation, title, titleLen, reply)
	returned = true
}

func lookup(handle C.uintptr_t, operation C.uint32_t, title *C.char, titleLen C.size_t, reply *C.rwikitext_v1_reply) {
	if uint64(titleLen) > uint64(^uint(0)>>1) {
		setReply(reply, C.RWIKITEXT_V1_ERROR, "lookup title too long")
		return
	}
	name := string(unsafe.Slice((*byte)(unsafe.Pointer(title)), int(titleLen)))
	lookups := cgo.Handle(handle).Value().(Lookups)
	var value any
	var found bool
	var err error
	switch operation {
	case C.RWIKITEXT_V1_TEMPLATE_TEXT:
		if lookups.TemplateText == nil {
			return
		}
		value, found, err = lookups.TemplateText(name)
	case C.RWIKITEXT_V1_PAGE_EXISTS:
		if lookups.PageExists == nil {
			return
		}
		value, found, err = lookups.PageExists(name)
	case C.RWIKITEXT_V1_FILE_INFO:
		if lookups.FileInfo == nil {
			return
		}
		value, found, err = lookups.FileInfo(name)
	default:
		setReply(reply, C.RWIKITEXT_V1_ERROR, "unknown lookup operation")
		return
	}
	if err != nil {
		setReply(reply, C.RWIKITEXT_V1_ERROR, err.Error())
		return
	}
	if found {
		setReply(reply, C.RWIKITEXT_V1_FOUND, value)
	}
}

func setReply(reply *C.rwikitext_v1_reply, status C.uint32_t, value any) {
	data, err := json.Marshal(value)
	if err != nil {
		data, _ = json.Marshal(fmt.Sprintf("encode lookup reply: %v", err))
		status = C.RWIKITEXT_V1_ERROR
	}
	payload := C.CString(string(data))
	defer C.free(unsafe.Pointer(payload))
	C.rwikitext_v1_reply_set(reply, status, payload)
}
