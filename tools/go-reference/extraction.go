package main

import (
	"bytes"
	"crypto/sha256"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"sort"
	"strings"

	"github.com/go-shiori/dom"
	"github.com/gogs/chardet"
	distiller "github.com/markusmobius/go-domdistiller"
	"github.com/markusmobius/go-domdistiller/data"
	"golang.org/x/net/html/charset"
	"golang.org/x/text/transform"
)

type extractionCase struct {
	HTML   string          `json:"html"`
	URL    string          `json:"url"`
	Root   string          `json:"root"`
	Title  string          `json:"title"`
	Markup data.MarkupInfo `json:"markup"`
	Words  int             `json:"words"`
	Node   string          `json:"node"`
	Text   string          `json:"text"`
	Images []string        `json:"images"`
}

func extractionCases(converters []converterCase, markup []markupCase) []extractionCase {
	inputs := []string{}
	for index, converter := range converters {
		if index%2 == 0 {
			inputs = append(inputs, converter.HTML)
		}
	}
	for index, item := range markup {
		if index%11 == 0 {
			inputs = append(inputs, item.HTML)
		}
	}
	for _, words := range []int{0, 1, 15, 16, 40, 41, 100, 499, 500, 501, 600} {
		for _, unlikely := range []string{"", "comment", "community", "article-comment", "sponsor"} {
			inputs = append(inputs, "<html><head><title>The full title for this article - Site</title></head><body><nav><a href='/'>Home</a></nav><article><h1>The full title for this article</h1><p>"+strings.Repeat("word ", words)+"</p><div class='"+unlikely+"'><p>"+strings.Repeat("additional content words ", 90)+"</p></div><figure><img src='cover.jpg'><figcaption>Caption.</figcaption></figure></article><footer>Copyright holder</footer></body></html>")
		}
	}
	cases := []extractionCase{}
	for _, input := range inputs {
		for _, rootSelector := range []string{"", "body"} {
			for _, pageURL := range []string{"", "https://example.com/news/story.html"} {
				parsed, err := dom.Parse(strings.NewReader(input))
				if err != nil {
					panic(err)
				}
				root := parsed
				if rootSelector != "" {
					root = dom.QuerySelector(parsed, rootSelector)
				}
				opts := &distiller.Options{SkipPagination: true}
				if pageURL != "" {
					opts.OriginalURL, err = url.Parse(pageURL)
					if err != nil {
						panic(err)
					}
				}
				result, err := distiller.Apply(root, opts)
				if err != nil {
					panic(err)
				}
				cases = append(cases, extractionCase{input, pageURL, rootSelector, result.Title, result.MarkupInfo, result.WordCount, dom.OuterHTML(result.Node), result.Text, result.ContentImages})
			}
		}
	}
	return cases
}

type readerCase struct {
	Name     string   `json:"name"`
	Input    []int    `json:"input"`
	Charsets []string `json:"charsets"`
	Parsed   string   `json:"parsed"`
	Error    string   `json:"error"`
	Title    string   `json:"title"`
	Node     string   `json:"node"`
	Text     string   `json:"text"`
}

func readerCases() []readerCase {
	inputs := []struct {
		name  string
		input []byte
	}{
		{"empty", nil}, {"ascii", []byte("<p>Plain article text.</p>")},
		{"utf8-normalization", []byte("<title>Cafe\u0301 title</title><p>co\u00adoperate cafe\u0301 \u1100\u1161.</p>")},
		{"invalid-utf8", []byte{'<', 'p', '>', 0xff, 0x80, 0xc0, '<', '/', 'p', '>'}},
		{"zeros", []byte{0, 0, 0, 0}},
	}
	for _, encoding := range []string{"utf-8", "windows-1252", "windows-1251", "koi8-r", "iso-8859-2", "iso-8859-7", "iso-8859-8", "iso-8859-9", "windows-1256", "shift_jis", "euc-jp", "euc-kr", "gb18030", "big5", "utf-16le", "utf-16be"} {
		var text string
		switch encoding {
		case "windows-1251", "koi8-r":
			text = "\u0421\u0442\u0430\u0442\u044c\u044f \u043e \u043d\u043e\u0432\u044b\u0445 \u043d\u0430\u0443\u0447\u043d\u044b\u0445 \u0438\u0441\u0441\u043b\u0435\u0434\u043e\u0432\u0430\u043d\u0438\u044f\u0445 \u0438 \u043d\u043e\u0432\u043e\u0441\u0442\u044f\u0445. "
		case "iso-8859-2":
			text = "Za\u017c\u00f3\u0142\u0107 g\u0119\u015bl\u0105 ja\u017a\u0144. Wiadomo\u015bci i informacje naukowe. "
		case "iso-8859-7":
			text = "\u0395\u03bb\u03bb\u03b7\u03bd\u03b9\u03ba\u03ac \u03bd\u03ad\u03b1 \u03ba\u03b1\u03b9 \u03b5\u03c0\u03b9\u03c3\u03c4\u03b7\u03bc\u03bf\u03bd\u03b9\u03ba\u03ad\u03c2 \u03c0\u03bb\u03b7\u03c1\u03bf\u03c6\u03bf\u03c1\u03af\u03b5\u03c2. "
		case "iso-8859-8":
			text = "\u05d7\u05d3\u05e9\u05d5\u05ea \u05d5\u05de\u05d9\u05d3\u05e2 \u05e2\u05dc \u05de\u05d7\u05e7\u05e8 \u05de\u05d3\u05e2\u05d9. "
		case "iso-8859-9":
			text = "T\u00fcrk\u00e7e haberler ve bilimsel ara\u015ft\u0131rmalar hakk\u0131nda bilgi. "
		case "windows-1256":
			text = "\u0623\u062e\u0628\u0627\u0631 \u0648\u0645\u0639\u0644\u0648\u0645\u0627\u062a \u0639\u0646 \u0627\u0644\u0628\u062d\u062b \u0627\u0644\u0639\u0644\u0645\u064a. "
		case "shift_jis", "euc-jp":
			text = "\u65b0\u3057\u3044\u79d1\u5b66\u7814\u7a76\u3068\u30cb\u30e5\u30fc\u30b9\u306b\u95a2\u3059\u308b\u8a18\u4e8b\u3067\u3059\u3002 "
		case "euc-kr":
			text = "\uc0c8\ub85c\uc6b4 \uacfc\ud559 \uc5f0\uad6c\uc640 \ub274\uc2a4\uc5d0 \uad00\ud55c \uae30\uc0ac\uc785\ub2c8\ub2e4. "
		case "gb18030", "big5":
			text = "\u79d1\u5b78\u7814\u7a76\u548c\u65b0\u805e\u7684\u8a73\u7d30\u4fe1\u606f\u3002 "
		default:
			text = "Les derni\u00e8res nouvelles et les r\u00e9sultats de la recherche scientifique. "
		}
		for _, declared := range []bool{false, true} {
			html := "<title>Reader article title</title>"
			if declared {
				html += "<meta charset='" + encoding + "'>"
			}
			html += "<article><p>" + strings.Repeat(text, 40) + "</p></article>"
			encoder, _ := charset.Lookup(encoding)
			encoded, _, err := transform.Bytes(encoder.NewEncoder(), []byte(html))
			if err != nil {
				panic(err)
			}
			inputs = append(inputs, struct {
				name  string
				input []byte
			}{encoding, encoded})
			if encoding == "utf-8" {
				inputs = append(inputs, struct {
					name  string
					input []byte
				}{encoding + "-bom", append([]byte{0xef, 0xbb, 0xbf}, encoded...)})
			}
			if encoding == "utf-16le" {
				inputs = append(inputs, struct {
					name  string
					input []byte
				}{encoding + "-bom", append([]byte{0xff, 0xfe}, encoded...)})
			}
			if encoding == "utf-16be" {
				inputs = append(inputs, struct {
					name  string
					input []byte
				}{encoding + "-bom", append([]byte{0xfe, 0xff}, encoded...)})
			}
		}
	}
	cases := []readerCase{}
	for _, input := range inputs {
		entry := readerCase{Name: input.name, Input: []int{}, Charsets: []string{}}
		for _, value := range input.input {
			entry.Input = append(entry.Input, int(value))
		}
		detected, err := chardet.NewHtmlDetector().DetectAll(input.input)
		if err == nil {
			var baseline string
			for index, candidate := range detected {
				if candidate.Confidence != detected[0].Confidence {
					break
				}
				entry.Charsets = append(entry.Charsets, candidate.Charset)
				decoded := decodeReference(input.input, candidate.Charset)
				if index == 0 {
					baseline = string(decoded)
				} else if baseline != string(decoded) {
					panic(fmt.Sprintf("reader %s has non-equivalent tied charsets %v", input.name, entry.Charsets))
				}
			}
			sort.Strings(entry.Charsets)
		}
		parsed, err := dom.Parse(bytes.NewReader(input.input))
		if err != nil {
			entry.Error = err.Error()
		} else {
			entry.Parsed = dom.OuterHTML(parsed)
			result, err := distiller.ApplyForReader(bytes.NewReader(input.input), &distiller.Options{SkipPagination: true})
			if err != nil {
				panic(err)
			}
			entry.Title, entry.Node, entry.Text = result.Title, dom.OuterHTML(result.Node), result.Text
		}
		cases = append(cases, entry)
	}
	return cases
}

type publicCase struct {
	Name       string              `json:"name"`
	HTML       string              `json:"html"`
	SHA256     string              `json:"sha256"`
	InputURL   string              `json:"input_url"`
	Skip       bool                `json:"skip"`
	Number     bool                `json:"number"`
	Result     extractionCase      `json:"result"`
	Pagination data.PaginationInfo `json:"pagination"`
}

func publicCases(pagination []paginationCase) []publicCase {
	directory, err := exec.Command(filepath.Join(runtime.GOROOT(), "bin", "go"), "list", "-mod=readonly", "-m", "-f", "{{.Dir}}", "github.com/markusmobius/go-domdistiller").Output()
	if err != nil {
		panic(err)
	}
	saved, err := os.ReadFile(filepath.Join(strings.TrimSpace(string(directory)), "example", "sample.html"))
	if err != nil {
		panic(err)
	}
	parsed, err := dom.Parse(bytes.NewReader(saved))
	if err != nil {
		panic(err)
	}
	canonical := dom.QuerySelector(parsed, "link[rel='canonical']")
	if canonical == nil {
		panic("saved page is missing its canonical link")
	}
	pageURL := dom.GetAttribute(canonical, "href")
	if pageURL == "" {
		panic("saved page is missing its original URL")
	}
	inputs := []struct{ name, html, url string }{{"upstream-saved-page", string(saved), pageURL}}
	for index := 0; index < len(pagination); index += 57 {
		item := pagination[index]
		inputs = append(inputs, struct{ name, html, url string }{fmt.Sprintf("pagination-%d", index), item.HTML, item.URL})
	}
	for _, inputURL := range []string{"HTTP://Example.COM:80/news/a b?q=a b#one two", "https://Example.COM", "http://example.com/a%2fb", "http:opaque", "/relative/path", ""} {
		inputs = append(inputs, struct{ name, html, url string }{"url", "<title>Article title</title><p>" + strings.Repeat("The latest article news. ", 50) + "<a href='next.html'>next page</a></p>", inputURL})
	}
	cases := []publicCase{}
	for _, input := range inputs {
		for _, skip := range []bool{false, true} {
			for _, number := range []bool{false, true} {
				options := &distiller.Options{SkipPagination: skip}
				options.OriginalURL, err = url.Parse(input.url)
				if err != nil {
					panic(err)
				}
				if number {
					options.PaginationAlgo = distiller.PageNumber
				}
				result, err := distiller.ApplyForReader(strings.NewReader(input.html), options)
				if err != nil {
					panic(err)
				}
				entry := extractionCase{HTML: "", URL: result.URL, Title: result.Title, Markup: result.MarkupInfo, Words: result.WordCount, Node: dom.OuterHTML(result.Node), Text: result.Text, Images: result.ContentImages}
				cases = append(cases, publicCase{input.name, input.html, fmt.Sprintf("%x", sha256.Sum256([]byte(input.html))), input.url, skip, number, entry, result.PaginationInfo})
			}
		}
	}
	return cases
}
