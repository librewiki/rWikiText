// Package rwikitext converts documents using the rWikiText native library.
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

// Format identifies a document format.
type Format string

const (
	Wikitext Format = "wikitext"
	NamuMark Format = "namumark"
	GFM      Format = "gfm"
	HTML     Format = "html"
)

// Mode controls how conversion loss is handled. The zero value is BestEffort.
type Mode string

const (
	BestEffort Mode = "best_effort"
	Strict     Mode = "strict"
)

// WikiContext supplies the page and site settings for a conversion.
type WikiContext struct {
	PageTitle     string     `json:"page_title,omitempty"`
	SiteLanguage  *string    `json:"site_language,omitempty"`
	ReferenceTime *time.Time `json:"reference_time,omitempty"` // Converted to UTC at whole-second precision.
}

// ExpansionLimits bounds transclusion; nil fields use the native defaults.
type ExpansionLimits struct {
	MaxDepth       *uint `json:"max_depth,omitempty"`
	MaxExpansions  *uint `json:"max_expansions,omitempty"`
	MaxOutputBytes *uint `json:"max_output_bytes,omitempty"`
}

// FileInfo describes a file returned by a lookup.
type FileInfo struct {
	URL    string  `json:"url"`
	Width  *uint32 `json:"width,omitempty"`
	Height *uint32 `json:"height,omitempty"`
}

// Lookups supplies wiki data. The boolean reports whether a value was found.
type Lookups struct {
	TemplateText func(string) (string, bool, error)
	PageExists   func(string) (bool, bool, error)
	FileInfo     func(string) (FileInfo, bool, error)
}

// Options configures conversion. Its zero value uses native defaults.
type Options struct {
	Context         WikiContext
	Mode            Mode
	ExpansionLimits ExpansionLimits
	Lookups         Lookups
}

// DiagnosticKind identifies the reason for a diagnostic.
type DiagnosticKind string

const (
	ConversionLoss DiagnosticKind = "conversion_loss"
	HTMLRemoval    DiagnosticKind = "html_removal"
	WikiLookup     DiagnosticKind = "wiki_lookup"
	ExpansionLimit DiagnosticKind = "expansion_limit"
)

// Diagnostic describes an issue in a half-open UTF-8 byte range of the input.
type Diagnostic struct {
	Kind    DiagnosticKind `json:"kind"`
	Message string         `json:"message"`
	Start   int            `json:"start"`
	End     int            `json:"end"`
}

// PageMetadata contains information outside the rendered page body.
type PageMetadata struct {
	Categories       []string  `json:"categories"`
	CategorySortKeys []*string `json:"category_sort_keys"`
	RedirectTarget   *string   `json:"redirect_target"`
	DisplayTitle     *string   `json:"display_title"`
}

// Result contains the converted document, metadata and diagnostics.
type Result struct {
	Content     string       `json:"content"`
	Metadata    PageMetadata `json:"metadata"`
	Diagnostics []Diagnostic `json:"diagnostics"`
}

// ConversionError is a failure reported by the converter.
type ConversionError struct {
	Message     string
	Diagnostics []Diagnostic
}

func (e *ConversionError) Error() string { return e.Message }

// NativeError reports a request encoding, ABI, or response processing failure.
type NativeError struct{ Message string }

func (e *NativeError) Error() string { return e.Message }

// Convert converts UTF-8 input from source to target using the supplied options.
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
