package main

import (
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"runtime/debug"
	"strings"

	"github.com/markusmobius/go-domdistiller/internal/filter/english"
	"github.com/markusmobius/go-domdistiller/internal/stringutil"
	"github.com/markusmobius/go-domdistiller/internal/webdoc"
)

const moduleVersion = "v0.0.0-20240926050704-25b8d046ffb4"
const sourceCommit = "25b8d046ffb4053bf68345d6fa59bc9ae1961ad8"

var dependencyVersions = map[string]string{
	"github.com/andybalholm/cascadia":         "v1.3.2",
	"github.com/go-shiori/dom":                "v0.0.0-20230515143342-73569d674e1c",
	"github.com/gogs/chardet":                 "v0.0.0-20211120154057-b7413eaefb8f",
	"github.com/markusmobius/go-domdistiller": moduleVersion,
	"github.com/mattn/go-colorable":           "v0.1.13",
	"github.com/mattn/go-isatty":              "v0.0.20",
	"github.com/rs/zerolog":                   "v1.33.0",
	"golang.org/x/net":                        "v0.29.0",
	"golang.org/x/sys":                        "v0.25.0",
	"golang.org/x/text":                       "v0.18.0",
}

type wordCase struct {
	Input    string `json:"input"`
	Full     int    `json:"full"`
	Letter   int    `json:"letter"`
	Fast     int    `json:"fast"`
	Selected string `json:"selected"`
}

type blockInput struct {
	Words   int     `json:"words"`
	Density float64 `json:"density"`
}

type classifierCase struct {
	Input   []blockInput `json:"input"`
	Content []bool       `json:"content"`
	Changed bool         `json:"changed"`
}

type fixture struct {
	GoVersion    string             `json:"go_version"`
	Version      string             `json:"version"`
	Commit       string             `json:"commit"`
	Words        []wordCase         `json:"words"`
	Classifiers  []classifierCase   `json:"classifiers"`
	Filters      []filterCase       `json:"filters"`
	DOM          []domCase          `json:"dom"`
	Converters   []converterCase    `json:"converters"`
	Siblings     []siblingCase      `json:"siblings"`
	Tables       []tableCase        `json:"tables"`
	Markup       []markupCase       `json:"markup"`
	Extraction   []extractionCase   `json:"extraction"`
	Pagination   []paginationCase   `json:"pagination"`
	NumberGroups []numberGroupsCase `json:"number_groups"`
	PagePatterns []pagePatternCase  `json:"page_patterns"`
	URLs         []urlCase          `json:"urls"`
	Readers      []readerCase       `json:"readers"`
	Encodings    []encodingCase     `json:"encodings"`
	Decodings    []decodingCase     `json:"decodings"`
	Public       []publicCase       `json:"public"`
}

func main() {
	output := flag.String("output", "../../testdata/go-reference.json", "output fixture")
	tablesOutput := flag.String("encoding-tables", "", "export pinned charset recognition tables instead of fixtures")
	noticesOutput := flag.String("notices", "", "export pinned upstream notices into the crate directory")
	benchmarkSource := flag.String("benchmark-source", "", "export the pinned content-extractor-benchmark corpus to -output")
	benchmarkInput := flag.String("benchmark", "", "run the JSON-line benchmark worker with this prepared corpus")
	flag.Parse()
	info, ok := debug.ReadBuildInfo()
	if !ok || runtime.Version() != "go1.27.1" {
		panic("reference requires Go 1.27.1")
	}
	for _, dependency := range info.Deps {
		if dependency.Replace != nil {
			panic("reference dependencies must not be replaced")
		}
		expected, ok := dependencyVersions[dependency.Path]
		if !ok || dependency.Version != expected {
			panic("unexpected reference dependency: " + dependency.Path + " " + dependency.Version)
		}
	}
	graph, err := exec.Command(filepath.Join(runtime.GOROOT(), "bin", "go"), "list", "-mod=readonly", "-m", "-json", "all").Output()
	if err != nil {
		panic(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(graph))
	found := map[string]bool{}
	for {
		var module struct {
			Path, Version string
			Replace       *json.RawMessage
		}
		if err := decoder.Decode(&module); err == io.EOF {
			break
		} else if err != nil {
			panic(err)
		}
		if module.Replace != nil {
			panic("reference dependencies must not be replaced")
		}
		if expected, ok := dependencyVersions[module.Path]; ok {
			if module.Version != expected {
				panic("unexpected reference module: " + module.Path + " " + module.Version)
			}
			found[module.Path] = true
		}
	}
	for name := range dependencyVersions {
		if !found[name] {
			panic("missing reference dependency: " + name)
		}
	}
	if *benchmarkSource != "" {
		exportBenchmark(*benchmarkSource, *output)
		return
	}
	if *benchmarkInput != "" {
		runBenchmark(*benchmarkInput)
		return
	}
	if *noticesOutput != "" {
		exportNotices(*noticesOutput)
		return
	}
	if *tablesOutput != "" {
		exportEncodingTables(*tablesOutput)
		return
	}
	result := fixture{GoVersion: runtime.Version(), Version: moduleVersion, Commit: sourceCommit}
	inputs := []string{"", "  -@# ';]", "word", "b'fore", " _word.under_score_ ", " \ttwo\nwords", "one\u00a0two", "one\vtwo\fthree", "a\x00\u5b57", "\ud55c\uad6d\uc5b4 \ub2e8\uc5b4", "word\u5b57", "word \u5b57"}
	for _, boundary := range []rune{0, 9, 11, 12, 32, 48, 57, 65, 90, 95, 97, 122, 0xbf, 0xc0, 0x1fff, 0x2000, 0x303f, 0x3040, 0xa4cf, 0xa4d0, 0xabff, 0xac00, 0xd7af, 0xd7b0, 0x20000} {
		for _, delta := range []rune{-1, 0, 1} {
			if boundary+delta >= 0 {
				inputs = append(inputs, string(boundary+delta), "a"+string(boundary+delta)+"b")
			}
		}
	}
	for length := 1; length <= 100; length++ {
		inputs = append(inputs, strings.Repeat("\u5b57", length))
	}
	for _, input := range inputs {
		selected := "fast"
		switch stringutil.SelectWordCounter(input).(type) {
		case stringutil.FullWordCounter:
			selected = "full"
		case stringutil.LetterWordCounter:
			selected = "letter"
		}
		result.Words = append(result.Words, wordCase{input, stringutil.FullWordCounter{}.Count(input), stringutil.LetterWordCounter{}.Count(input), stringutil.FastWordCounter{}.Count(input), selected})
	}
	for _, currentWords := range []int{0, 1, 4, 5, 15, 16, 17, 18, 40, 41, 100} {
		for _, currentDensity := range []float64{0, 0.333332, 0.333333, 0.333334, 1} {
			for _, previousWords := range []int{-1, 0, 4, 5, 40} {
				for _, previousDensity := range []float64{0, 0.555555, 0.555556, 0.555557, 1} {
					for _, nextWords := range []int{-1, 0, 15, 16, 17, 18} {
						inputs := []blockInput{}
						if previousWords >= 0 {
							inputs = append(inputs, blockInput{previousWords, previousDensity})
						}
						inputs = append(inputs, blockInput{currentWords, currentDensity})
						if nextWords >= 0 {
							inputs = append(inputs, blockInput{nextWords, 0})
						}
						blocks := []*webdoc.TextBlock{}
						for _, input := range inputs {
							blocks = append(blocks, &webdoc.TextBlock{NumWords: input.Words, LinkDensity: input.Density})
						}
						document := webdoc.NewTextDocument(blocks)
						changed := english.NewNumWordsRulesClassifier().Process(document)
						content := []bool{}
						for _, block := range blocks {
							content = append(content, block.IsContent())
						}
						result.Classifiers = append(result.Classifiers, classifierCase{inputs, content, changed})
					}
				}
			}
		}
	}
	result.Filters = filterCases()
	result.DOM = domCases()
	result.Converters = converterCases()
	result.Siblings = siblingCases()
	result.Tables = tableCases()
	result.Markup = markupCases()
	result.Extraction = extractionCases(result.Converters, result.Markup)
	result.Pagination = paginationCases()
	result.NumberGroups = numberGroupsCases()
	result.PagePatterns = pagePatternCases()
	result.URLs = urlCases()
	result.Readers = readerCases()
	result.Encodings = encodingCases(result.Readers)
	result.Decodings = decodingCases()
	result.Public = publicCases(result.Pagination)
	encoded, err := json.MarshalIndent(result, "", "  ")
	if err != nil {
		panic(err)
	}
	if err := os.MkdirAll(filepath.Dir(*output), 0755); err != nil {
		panic(err)
	}
	if err := os.WriteFile(*output, append(encoded, '\n'), 0644); err != nil {
		panic(err)
	}
	fmt.Printf("Exported %d word cases and %d classifier cases from Go %s\n", len(result.Words), len(result.Classifiers), sourceCommit)
}
