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

// WikiContext supplies page- and site-level settings for a conversion.
//
// MediaWiki magic words such as {{PAGENAME}}, {{CONTENTLANGUAGE}}, and
// {{CURRENTTIMESTAMP}} read from these fields. Zero or nil fields use the native
// defaults: empty page title, site language "en", and reference time 1970-01-01T00:00:00Z.
type WikiContext struct {
	// PageTitle is the title of the page being converted (for example, "Guide").
	PageTitle string `json:"page_title,omitempty"`
	// SiteLanguage is the BCP 47 / MediaWiki content language code (for example, "en" or "ko").
	SiteLanguage *string `json:"site_language,omitempty"`
	// ReferenceTime is converted to UTC at whole-second precision before being sent to the converter.
	ReferenceTime *time.Time `json:"reference_time,omitempty"`
}

// ExpansionLimits bounds MediaWiki template and parser-function expansion.
//
// Nil fields use the native defaults: MaxDepth 40, MaxExpansions 10000, and
// MaxOutputBytes 2097152 (2 MiB).
type ExpansionLimits struct {
	// MaxDepth limits recursive template nesting depth.
	MaxDepth *uint `json:"max_depth,omitempty"`
	// MaxExpansions limits the total number of template and parser-function expansions.
	MaxExpansions *uint `json:"max_expansions,omitempty"`
	// MaxOutputBytes limits the expanded wikitext buffer size in bytes.
	MaxOutputBytes *uint `json:"max_output_bytes,omitempty"`
}

// FileInfo describes rendering metadata for a wiki file returned by [Lookups.FileInfo].
type FileInfo struct {
	// URL is the absolute or root-relative URL for the file resource.
	URL string `json:"url"`
	// Width is the optional intrinsic image width in pixels.
	Width *uint32 `json:"width,omitempty"`
	// Height is the optional intrinsic image height in pixels.
	Height *uint32 `json:"height,omitempty"`
}

// Lookups supplies synchronous wiki-data callbacks invoked on the calling goroutine
// during MediaWiki conversion.
//
// Each callback receives a canonical namespace-prefixed, underscore-separated title
// (for example, "Template:Greeting", "Main_Page", or "File:Example.png").
// The returned boolean reports whether a value was found; returning (zero, false, nil)
// or leaving a callback nil reports missing data. Returning a non-nil error or panicking
// aborts the conversion with a located [WikiLookup] diagnostic.
//
// NamuMark input does not invoke any callbacks.
type Lookups struct {
	// TemplateText returns the raw wikitext source of a transcluded template.
	TemplateText func(title string) (text string, found bool, err error)
	// PageExists reports whether a page exists. Returning (false, true, nil) explicitly
	// marks the page absent (producing red links in HTML output).
	PageExists func(title string) (exists bool, found bool, err error)
	// FileInfo returns rendering metadata for a wiki file.
	FileInfo func(title string) (info FileInfo, found bool, err error)
}

// Options configures a call to [Convert]. Its zero value uses native defaults
// ([BestEffort] mode, default [WikiContext], default [ExpansionLimits], and no lookups).
type Options struct {
	// Context supplies page title, site language, and reference time.
	Context WikiContext
	// Mode selects [BestEffort] or [Strict] loss handling.
	Mode Mode
	// ExpansionLimits bounds MediaWiki template expansion.
	ExpansionLimits ExpansionLimits
	// Lookups provides optional synchronous wiki-data callbacks.
	Lookups Lookups
}

// DiagnosticKind identifies the category of a [Diagnostic].
type DiagnosticKind string

const (
	// ConversionLoss indicates a source construct could not be represented losslessly.
	ConversionLoss DiagnosticKind = "conversion_loss"
	// HTMLRemoval indicates unsafe or disallowed raw HTML was stripped.
	HTMLRemoval DiagnosticKind = "html_removal"
	// WikiLookup indicates a [Lookups] callback returned an error, panicked, or produced an invalid reply.
	WikiLookup DiagnosticKind = "wiki_lookup"
	// ExpansionLimit indicates MediaWiki expansion exceeded a configured [ExpansionLimits] bound.
	ExpansionLimit DiagnosticKind = "expansion_limit"
)

// Diagnostic describes a located warning or failure in a half-open UTF-8 byte
// range [Start, End) of the original input.
type Diagnostic struct {
	// Kind classifies the diagnostic.
	Kind DiagnosticKind `json:"kind"`
	// Message is a human-readable description of the issue.
	Message string `json:"message"`
	// Start is the inclusive UTF-8 byte offset in the input.
	Start int `json:"start"`
	// End is the exclusive UTF-8 byte offset in the input.
	End int `json:"end"`
}

// PageMetadata contains page-level metadata extracted outside the rendered body.
type PageMetadata struct {
	// Categories lists category names in document order.
	Categories []string `json:"categories"`
	// CategorySortKeys is parallel to Categories; nil entries indicate no explicit sort key.
	CategorySortKeys []*string `json:"category_sort_keys"`
	// RedirectTarget is the redirect destination page title, or nil if not a redirect.
	RedirectTarget *string `json:"redirect_target"`
	// DisplayTitle is the custom display title from {{DISPLAYTITLE:...}}, or nil if unset.
	DisplayTitle *string `json:"display_title"`
}

// Result contains the converted document, extracted metadata, and non-fatal diagnostics.
type Result struct {
	// Content is the converted document text in the requested target format.
	Content string `json:"content"`
	// Metadata holds page-level metadata extracted from the source document.
	Metadata PageMetadata `json:"metadata"`
	// Diagnostics lists any non-fatal warnings emitted during conversion.
	Diagnostics []Diagnostic `json:"diagnostics"`
}

// ConversionError is returned by [Convert] when the converter rejects or aborts a conversion,
// such as for unsupported format pairs, [Strict] conversion loss, [WikiLookup] failures,
// or exceeded [ExpansionLimits].
type ConversionError struct {
	// Message describes why the conversion failed.
	Message string
	// Diagnostics contains the located diagnostics associated with the failure, if any.
	Diagnostics []Diagnostic
}

// Error implements the error interface.
func (e *ConversionError) Error() string { return e.Message }

// NativeError is returned by [Convert] when the input is not valid UTF-8, the linked
// native ABI version mismatches, or JSON request/response marshaling fails.
type NativeError struct {
	// Message describes the binding or ABI failure.
	Message string
}

// Error implements the error interface.
func (e *NativeError) Error() string { return e.Message }

// Convert converts UTF-8 input from source to target using the supplied options.
//
// Convert is safe for concurrent use across goroutines and supports nested
// (re-entrant) calls from inside [Lookups] callbacks.
//
// Returned errors are either [*ConversionError] (converter-reported failure with
// optional located diagnostics) or [*NativeError] (invalid UTF-8 input, ABI mismatch,
// or marshaling failure).
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
