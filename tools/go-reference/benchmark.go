package main

import (
	"bufio"
	"bytes"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"io"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/go-shiori/dom"
	"github.com/gogs/chardet"
	distiller "github.com/markusmobius/go-domdistiller"
	"golang.org/x/net/html"
)

const benchmarkCommit = "466fdbee8a504441eb78ed11d71c1da220681cab"

type benchmarkPage struct {
	File       string   `json:"file"`
	URL        string   `json:"url"`
	HTML       string   `json:"html"`
	With       []string `json:"with"`
	Without    []string `json:"without"`
	SHA256     string   `json:"raw_sha256"`
	UTF8SHA256 string   `json:"utf8_sha256"`
	Charsets   []string `json:"charsets"`
}

func benchmarkString(expression ast.Expr) string {
	value, ok := expression.(*ast.BasicLit)
	if !ok || value.Kind != token.STRING {
		panic("benchmark string is not a literal")
	}
	text, err := strconv.Unquote(value.Value)
	if err != nil {
		panic(err)
	}
	return text
}

func benchmarkStrings(expression ast.Expr) []string {
	value, ok := expression.(*ast.CompositeLit)
	if !ok {
		panic("benchmark list is not a composite literal")
	}
	result := []string{}
	for _, entry := range value.Elts {
		result = append(result, benchmarkString(entry))
	}
	return result
}

func exportBenchmark(source, output string) {
	revision, err := exec.Command("git", "-C", source, "rev-parse", "HEAD").Output()
	if err != nil || strings.TrimSpace(string(revision)) != benchmarkCommit {
		panic("benchmark source must be checked out at " + benchmarkCommit)
	}
	if err := exec.Command("git", "-C", source, "diff", "--quiet", "HEAD", "--").Run(); err != nil {
		panic("benchmark source has modified tracked files")
	}
	parsed, err := parser.ParseFile(token.NewFileSet(), filepath.Join(source, "data.go"), nil, 0)
	if err != nil {
		panic(err)
	}
	var entries *ast.CompositeLit
	ast.Inspect(parsed, func(node ast.Node) bool {
		value, ok := node.(*ast.ValueSpec)
		if ok && len(value.Names) == 1 && value.Names[0].Name == "comparisonData" {
			entries = value.Values[0].(*ast.CompositeLit)
		}
		return true
	})
	if entries == nil {
		panic("missing benchmark comparisonData")
	}
	pages := []benchmarkPage{}
	for _, expression := range entries.Elts {
		page := benchmarkPage{With: []string{}, Without: []string{}, Charsets: []string{}}
		for _, field := range expression.(*ast.CompositeLit).Elts {
			pair := field.(*ast.KeyValueExpr)
			switch pair.Key.(*ast.Ident).Name {
			case "File":
				page.File = benchmarkString(pair.Value)
			case "URL":
				page.URL = benchmarkString(pair.Value)
			case "With":
				page.With = benchmarkStrings(pair.Value)
			case "Without":
				page.Without = benchmarkStrings(pair.Value)
			}
		}
		pageURL, err := url.ParseRequestURI(page.URL)
		if err != nil {
			panic(fmt.Errorf("%s: %w", page.File, err))
		}
		page.URL = pageURL.String()
		if !filepath.IsLocal(page.File) {
			panic("non-local corpus filename: " + page.File)
		}
		raw, err := os.ReadFile(filepath.Join(source, "files", page.File))
		if err != nil {
			panic(err)
		}
		detected, err := chardet.NewHtmlDetector().DetectAll(raw)
		if err != nil || len(detected) == 0 {
			panic("charset not detected for " + page.File)
		}
		normalized := decodeReference(raw, detected[0].Charset)
		for _, candidate := range detected {
			if candidate.Confidence != detected[0].Confidence {
				break
			}
			page.Charsets = append(page.Charsets, candidate.Charset)
			if !bytes.Equal(normalized, decodeReference(raw, candidate.Charset)) {
				panic("ambiguous charset changes decoded input: " + page.File)
			}
		}
		sort.Strings(page.Charsets)
		page.SHA256 = fmt.Sprintf("%x", sha256.Sum256(raw))
		page.UTF8SHA256 = fmt.Sprintf("%x", sha256.Sum256(normalized))
		page.HTML = string(normalized)
		pages = append(pages, page)
	}
	encoded, err := json.Marshal(pages)
	if err != nil {
		panic(err)
	}
	if err := os.MkdirAll(filepath.Dir(output), 0755); err != nil {
		panic(err)
	}
	if err := os.WriteFile(output, append(encoded, '\n'), 0644); err != nil {
		panic(err)
	}
	fmt.Printf("Exported %d benchmark pages at %s\n", len(pages), benchmarkCommit)
}

type benchmarkCounts struct {
	TruePositives  int `json:"true_positives"`
	FalseNegatives int `json:"false_negatives"`
	FalsePositives int `json:"false_positives"`
	TrueNegatives  int `json:"true_negatives"`
}

func (counts *benchmarkCounts) evaluate(text string, page benchmarkPage) {
	for _, snippet := range page.With {
		if text != "" && strings.Contains(text, snippet) {
			counts.TruePositives++
		} else {
			counts.FalseNegatives++
		}
	}
	for _, snippet := range page.Without {
		if text != "" && strings.Contains(text, snippet) {
			counts.FalsePositives++
		} else {
			counts.TrueNegatives++
		}
	}
}

type benchmarkRequest struct {
	Pagination string `json:"pagination"`
	Outputs    bool   `json:"outputs"`
}

type benchmarkOutput struct {
	File     string `json:"file"`
	Text     string `json:"text"`
	Title    string `json:"title"`
	Words    int    `json:"words"`
	HTML     string `json:"html"`
	NextPage string `json:"next_page"`
	PrevPage string `json:"prev_page"`
}

type benchmarkResponse struct {
	ElapsedNS int64             `json:"elapsed_ns"`
	Counts    benchmarkCounts   `json:"counts"`
	Errors    []string          `json:"errors"`
	Outputs   []benchmarkOutput `json:"outputs"`
}

func runBenchmark(input string) {
	runtime.GOMAXPROCS(1)
	file, err := os.Open(input)
	if err != nil {
		panic(err)
	}
	var pages []benchmarkPage
	if err := json.NewDecoder(file).Decode(&pages); err != nil {
		panic(err)
	}
	file.Close()
	documents := make([]*html.Node, len(pages))
	options := make([]distiller.Options, len(pages))
	for index, page := range pages {
		documents[index], err = dom.FastParse(strings.NewReader(page.HTML))
		if err != nil {
			panic(err)
		}
		options[index].OriginalURL, err = url.ParseRequestURI(page.URL)
		if err != nil {
			panic(err)
		}
	}
	reader := json.NewDecoder(os.Stdin)
	writer := bufio.NewWriter(os.Stdout)
	encoder := json.NewEncoder(writer)
	if err := encoder.Encode(map[string]int{"ready": len(pages)}); err != nil {
		panic(err)
	}
	writer.Flush()
	for {
		var request benchmarkRequest
		if err := reader.Decode(&request); err == io.EOF {
			return
		} else if err != nil {
			panic(err)
		}
		var algorithm distiller.PaginationAlgo
		switch request.Pagination {
		case "skip", "prev-next":
			algorithm = distiller.PrevNext
		case "page-number":
			algorithm = distiller.PageNumber
		default:
			panic("unknown pagination mode: " + request.Pagination)
		}
		for index := range options {
			options[index].SkipPagination = request.Pagination == "skip"
			options[index].PaginationAlgo = algorithm
		}
		response := benchmarkResponse{Errors: []string{}, Outputs: []benchmarkOutput{}}
		started := time.Now()
		for index, page := range pages {
			result, err := distiller.Apply(documents[index], &options[index])
			if err != nil {
				response.Errors = append(response.Errors, fmt.Sprintf("%s: %v", page.File, err))
				response.Counts.evaluate("", page)
				continue
			}
			response.Counts.evaluate(result.Text, page)
			if request.Outputs {
				response.Outputs = append(response.Outputs, benchmarkOutput{
					File: page.File, Text: result.Text, Title: result.Title, Words: result.WordCount,
					HTML: dom.OuterHTML(result.Node), NextPage: result.PaginationInfo.NextPage, PrevPage: result.PaginationInfo.PrevPage,
				})
			}
		}
		response.ElapsedNS = time.Since(started).Nanoseconds()
		if err := encoder.Encode(response); err != nil {
			panic(err)
		}
		if err := writer.Flush(); err != nil {
			panic(err)
		}
	}
}
