// gen_godoc compiles Go package doc comments and Example tests into a static HTML page
// using only the Go standard library (go/parser, go/doc, go/doc/comment).
package main

import (
	"bytes"
	"flag"
	"fmt"
	"go/ast"
	"go/doc"
	"go/doc/comment"
	"go/format"
	"go/parser"
	"go/token"
	"html/template"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

type exampleView struct {
	Name   string
	Doc    template.HTML
	Code   string
	Output string
}

type funcView struct {
	Name     string
	Anchor   string
	Decl     string
	Doc      template.HTML
	Examples []exampleView
}

type valueView struct {
	Decl string
	Doc  template.HTML
}

type typeView struct {
	Name     string
	Decl     string
	Doc      template.HTML
	Consts   []valueView
	Vars     []valueView
	Funcs    []funcView
	Methods  []funcView
	Examples []exampleView
}

type pageView struct {
	PackageName string
	ImportPath  string
	Doc         template.HTML
	Consts      []valueView
	Vars        []valueView
	Funcs       []funcView
	Types       []typeView
	Examples    []exampleView
}

func formatNode(fset *token.FileSet, node any) string {
	var buf bytes.Buffer
	if err := format.Node(&buf, fset, node); err != nil {
		return ""
	}
	return buf.String()
}

func renderDoc(p *doc.Package, text string) template.HTML {
	if strings.TrimSpace(text) == "" {
		return ""
	}
	parsed := p.Parser().Parse(text)
	pr := &comment.Printer{
		DocLinkURL: func(link *comment.DocLink) string {
			if link.ImportPath == "" {
				if link.Recv != "" {
					return "#" + link.Recv + "." + link.Name
				}
				return "#" + link.Name
			}
			return "https://pkg.go.dev/" + link.ImportPath + "#" + link.Name
		},
	}
	return template.HTML(pr.HTML(parsed))
}

func collectExamples(p *doc.Package, fset *token.FileSet, all []*doc.Example, prefix string) []exampleView {
	var out []exampleView
	for _, ex := range all {
		matched := false
		if prefix == "" {
			matched = ex.Name == "" || ex.Suffix != "" && ex.Name == ex.Suffix
		} else {
			matched = ex.Name == prefix || strings.HasPrefix(ex.Name, prefix+"_")
		}
		if !matched {
			continue
		}
		var codeNode any = ex.Code
		if ex.Play != nil {
			codeNode = ex.Play
		}
		title := "Example"
		if ex.Name != "" {
			title = "Example (" + ex.Name + ")"
		}
		out = append(out, exampleView{
			Name:   title,
			Doc:    renderDoc(p, ex.Doc),
			Code:   strings.TrimSpace(formatNode(fset, codeNode)),
			Output: strings.TrimSpace(ex.Output),
		})
	}
	return out
}

func readModulePath(srcDir string) string {
	data, err := os.ReadFile(filepath.Join(srcDir, "go.mod"))
	if err != nil {
		return ""
	}
	for _, line := range strings.Split(string(data), "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "module ") {
			return strings.TrimSpace(strings.TrimPrefix(line, "module "))
		}
	}
	return ""
}

func main() {
	srcDir := flag.String("src", "bindings/go", "Go package source directory")
	outPath := flag.String("out", "target/api-docs/go/index.html", "Output HTML file path")
	flag.Parse()

	fset := token.NewFileSet()
	entries, err := os.ReadDir(*srcDir)
	if err != nil {
		fmt.Fprintf(os.Stderr, "read %s: %v\n", *srcDir, err)
		os.Exit(1)
	}

	var pkgFiles []*ast.File
	var testFiles []*ast.File
	var pkgName string

	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".go") {
			continue
		}
		fullPath := filepath.Join(*srcDir, entry.Name())
		file, err := parser.ParseFile(fset, fullPath, nil, parser.ParseComments)
		if err != nil {
			fmt.Fprintf(os.Stderr, "parse %s: %v\n", fullPath, err)
			os.Exit(1)
		}
		if strings.HasSuffix(entry.Name(), "_test.go") {
			testFiles = append(testFiles, file)
		} else {
			pkgFiles = append(pkgFiles, file)
			if pkgName == "" {
				pkgName = file.Name.Name
			}
		}
	}

	if len(pkgFiles) == 0 {
		fmt.Fprintf(os.Stderr, "no Go source files found in %s\n", *srcDir)
		os.Exit(1)
	}

	importPath := readModulePath(*srcDir)
	pkg, err := doc.NewFromFiles(fset, pkgFiles, importPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "doc.NewFromFiles: %v\n", err)
		os.Exit(1)
	}

	examples := doc.Examples(testFiles...)
	sort.Slice(examples, func(i, j int) bool { return examples[i].Name < examples[j].Name })

	convertValues := func(vals []*doc.Value) []valueView {
		var res []valueView
		for _, v := range vals {
			res = append(res, valueView{
				Decl: formatNode(fset, v.Decl),
				Doc:  renderDoc(pkg, v.Doc),
			})
		}
		return res
	}

	convertFuncs := func(funcs []*doc.Func, recvPrefix string) []funcView {
		var res []funcView
		for _, fn := range funcs {
			anchor := fn.Name
			exPrefix := fn.Name
			if recvPrefix != "" {
				anchor = recvPrefix + "." + fn.Name
				exPrefix = recvPrefix + "_" + fn.Name
			}
			res = append(res, funcView{
				Name:     fn.Name,
				Anchor:   anchor,
				Decl:     formatNode(fset, fn.Decl),
				Doc:      renderDoc(pkg, fn.Doc),
				Examples: collectExamples(pkg, fset, examples, exPrefix),
			})
		}
		return res
	}

	view := pageView{
		PackageName: pkg.Name,
		ImportPath:  importPath,
		Doc:         renderDoc(pkg, pkg.Doc),
		Consts:      convertValues(pkg.Consts),
		Vars:        convertValues(pkg.Vars),
		Funcs:       convertFuncs(pkg.Funcs, ""),
		Examples:    collectExamples(pkg, fset, examples, ""),
	}

	for _, t := range pkg.Types {
		view.Types = append(view.Types, typeView{
			Name:     t.Name,
			Decl:     formatNode(fset, t.Decl),
			Doc:      renderDoc(pkg, t.Doc),
			Consts:   convertValues(t.Consts),
			Vars:     convertValues(t.Vars),
			Funcs:    convertFuncs(t.Funcs, ""),
			Methods:  convertFuncs(t.Methods, t.Name),
			Examples: collectExamples(pkg, fset, examples, t.Name),
		})
	}

	tmpl := template.Must(template.New("godoc").Parse(htmlTemplate))
	if err := os.MkdirAll(filepath.Dir(*outPath), 0o755); err != nil {
		fmt.Fprintf(os.Stderr, "mkdir %s: %v\n", filepath.Dir(*outPath), err)
		os.Exit(1)
	}
	f, err := os.Create(*outPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "create %s: %v\n", *outPath, err)
		os.Exit(1)
	}
	defer f.Close()

	if err := tmpl.Execute(f, view); err != nil {
		fmt.Fprintf(os.Stderr, "render template: %v\n", err)
		os.Exit(1)
	}
}

const htmlTemplate = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>package {{.PackageName}} — rWikiText Go API</title>
  <style>
    :root {
      color-scheme: light dark;
      --bg: #ffffff;
      --surface: #f8fafc;
      --border: #e2e8f0;
      --text: #0f172a;
      --muted: #475569;
      --accent: #007d9c;
      --code-bg: #f1f5f9;
    }
    @media (prefers-color-scheme: dark) {
      :root {
        --bg: #0b1120;
        --surface: #111827;
        --border: #1f2937;
        --text: #f1f5f9;
        --muted: #94a3b8;
        --accent: #38bdf8;
        --code-bg: #1e293b;
      }
    }
    body {
      margin: 0;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      background: var(--bg);
      color: var(--text);
      line-height: 1.6;
    }
    .wrap {
      max-width: 960px;
      margin: 0 auto;
      padding: 32px 24px 64px;
    }
    a { color: var(--accent); text-decoration: none; }
    a:hover { text-decoration: underline; }
    header {
      border-bottom: 1px solid var(--border);
      padding-bottom: 16px;
      margin-bottom: 24px;
    }
    .back { font-size: 0.9rem; display: inline-block; margin-bottom: 8px; }
    h1 { margin: 0 0 8px; font-size: 2rem; }
    h2 { margin-top: 36px; border-bottom: 1px solid var(--border); padding-bottom: 6px; }
    h3 { margin-top: 24px; }
    .import-path {
      display: inline-block;
      padding: 4px 10px;
      border-radius: 6px;
      background: var(--code-bg);
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 0.9rem;
    }
    pre {
      background: var(--code-bg);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 14px 16px;
      overflow-x: auto;
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 0.88rem;
      line-height: 1.45;
    }
    code {
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 0.9em;
    }
    .toc {
      background: var(--surface);
      border: 1px solid var(--border);
      border-radius: 10px;
      padding: 16px 20px;
      margin: 24px 0;
    }
    .toc ul { margin: 8px 0 0; padding-left: 20px; }
    details.example {
      margin: 12px 0 20px;
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 10px 14px;
      background: var(--surface);
    }
    details.example summary {
      cursor: pointer;
      font-weight: 600;
      color: var(--accent);
    }
  </style>
</head>
<body>
  <main class="wrap">
    <header>
      <a class="back" href="../index.html">&larr; All Languages</a>
      <h1>package {{.PackageName}}</h1>
      {{if .ImportPath}}<div class="import-path">import "{{.ImportPath}}"</div>{{end}}
    </header>

    <section id="overview">
      <h2>Overview</h2>
      {{.Doc}}
    </section>

    <nav class="toc" aria-label="Index">
      <strong>Index</strong>
      <ul>
        {{if .Consts}}<li><a href="#constants">Constants</a></li>{{end}}
        {{if .Vars}}<li><a href="#variables">Variables</a></li>{{end}}
        {{range .Funcs}}<li><a href="#{{.Anchor}}">func {{.Name}}</a></li>{{end}}
        {{range .Types}}
          {{$typeName := .Name}}
          <li><a href="#{{$typeName}}">type {{$typeName}}</a>
            {{if or .Funcs .Methods}}
            <ul>
              {{range .Funcs}}<li><a href="#{{.Anchor}}">func {{.Name}}</a></li>{{end}}
              {{range .Methods}}<li><a href="#{{.Anchor}}">func ({{$typeName}}) {{.Name}}</a></li>{{end}}
            </ul>
            {{end}}
          </li>
        {{end}}
      </ul>
    </nav>

    {{if .Consts}}
    <section id="constants">
      <h2>Constants</h2>
      {{range .Consts}}
        <pre><code>{{.Decl}}</code></pre>
        {{.Doc}}
      {{end}}
    </section>
    {{end}}

    {{if .Vars}}
    <section id="variables">
      <h2>Variables</h2>
      {{range .Vars}}
        <pre><code>{{.Decl}}</code></pre>
        {{.Doc}}
      {{end}}
    </section>
    {{end}}

    {{if .Funcs}}
    <section id="functions">
      <h2>Functions</h2>
      {{range .Funcs}}
        <article id="{{.Anchor}}">
          <h3>func {{.Name}}</h3>
          <pre><code>{{.Decl}}</code></pre>
          {{.Doc}}
          {{range .Examples}}
            <details class="example" open>
              <summary>{{.Name}}</summary>
              {{.Doc}}
              <pre><code>{{.Code}}</code></pre>
              {{if .Output}}<p><strong>Output:</strong></p><pre><code>{{.Output}}</code></pre>{{end}}
            </details>
          {{end}}
        </article>
      {{end}}
    </section>
    {{end}}

    {{if .Types}}
    <section id="types">
      <h2>Types</h2>
      {{range .Types}}
        <article id="{{.Name}}">
          <h3>type {{.Name}}</h3>
          <pre><code>{{.Decl}}</code></pre>
          {{.Doc}}
          {{range .Consts}}
            <pre><code>{{.Decl}}</code></pre>
            {{.Doc}}
          {{end}}
          {{range .Vars}}
            <pre><code>{{.Decl}}</code></pre>
            {{.Doc}}
          {{end}}
          {{range .Funcs}}
            <article id="{{.Anchor}}">
              <h4>func {{.Name}}</h4>
              <pre><code>{{.Decl}}</code></pre>
              {{.Doc}}
            </article>
          {{end}}
          {{range .Methods}}
            <article id="{{.Anchor}}">
              <h4>func {{.Name}}</h4>
              <pre><code>{{.Decl}}</code></pre>
              {{.Doc}}
            </article>
          {{end}}
          {{range .Examples}}
            <details class="example" open>
              <summary>{{.Name}}</summary>
              {{.Doc}}
              <pre><code>{{.Code}}</code></pre>
              {{if .Output}}<p><strong>Output:</strong></p><pre><code>{{.Output}}</code></pre>{{end}}
            </details>
          {{end}}
        </article>
      {{end}}
    </section>
    {{end}}
  </main>
</body>
</html>
`
