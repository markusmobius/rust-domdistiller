package main

import (
	"fmt"
	"math/rand"
	"sort"
	"strings"

	"github.com/go-shiori/dom"
	"github.com/markusmobius/go-domdistiller/internal/converter"
	"github.com/markusmobius/go-domdistiller/internal/filter"
	"github.com/markusmobius/go-domdistiller/internal/filter/english"
	"github.com/markusmobius/go-domdistiller/internal/filter/heuristic"
	"github.com/markusmobius/go-domdistiller/internal/filter/simple"
	"github.com/markusmobius/go-domdistiller/internal/label"
	"github.com/markusmobius/go-domdistiller/internal/stringutil"
	"github.com/markusmobius/go-domdistiller/internal/webdoc"
)

type blockSnapshot struct {
	Text        string   `json:"text"`
	Labels      []string `json:"labels"`
	Words       int      `json:"words"`
	AnchorWords int      `json:"anchor_words"`
	Density     float64  `json:"density"`
	Level       int      `json:"level"`
	Content     bool     `json:"content"`
	Elements    []int    `json:"elements"`
}

type filterCase struct {
	Name    string          `json:"name"`
	Input   []blockSnapshot `json:"input"`
	Output  []blockSnapshot `json:"output"`
	Titles  []string        `json:"titles"`
	Changed bool            `json:"changed"`
}

func decodeBlocks(input []blockSnapshot) *webdoc.TextDocument {
	blocks := make([]*webdoc.TextBlock, 0, len(input))
	for _, source := range input {
		block := &webdoc.TextBlock{Text: source.Text, NumWords: source.Words, NumWordsInAnchor: source.AnchorWords, LinkDensity: source.Density, TagLevel: source.Level}
		block.SetIsContent(source.Content)
		block.AddLabels(source.Labels...)
		for _, offset := range source.Elements {
			block.TextElements = append(block.TextElements, &webdoc.Text{OffsetBlock: offset})
		}
		blocks = append(blocks, block)
	}
	return webdoc.NewTextDocument(blocks)
}

func encodeBlocks(document *webdoc.TextDocument) []blockSnapshot {
	result := []blockSnapshot{}
	for _, block := range document.TextBlocks {
		labels := []string{}
		for name := range block.Labels {
			labels = append(labels, name)
		}
		sort.Strings(labels)
		elements := []int{}
		for _, element := range block.TextElements {
			elements = append(elements, element.OffsetBlock)
		}
		result = append(result, blockSnapshot{block.Text, labels, block.NumWords, block.NumWordsInAnchor, block.LinkDensity, block.TagLevel, block.IsContent(), elements})
	}
	return result
}

type siblingCase struct {
	HTML          string          `json:"html"`
	Input         []blockSnapshot `json:"input"`
	Output        []blockSnapshot `json:"output"`
	Largest       bool            `json:"largest"`
	CrossTitles   bool            `json:"cross_titles"`
	CrossHeadings bool            `json:"cross_headings"`
	MixedTags     bool            `json:"mixed_tags"`
	MaxDensity    float64         `json:"max_density"`
	MaxDistance   int             `json:"max_distance"`
	Changed       bool            `json:"changed"`
}

func siblingCases() []siblingCase {
	result := []siblingCase{}
	random := rand.New(rand.NewSource(20260915))
	for sequence := 0; sequence < 80; sequence++ {
		var input strings.Builder
		input.WriteString("<article>")
		for index := 0; index < sequence%15; index++ {
			tag := []string{"p", "div", "h2", "section"}[random.Intn(4)]
			if index%3 == 0 {
				input.WriteString("<div>")
			}
			fmt.Fprintf(&input, "<%s>one two <b>three four</b> <a href='/a'>link</a> five</%s>", tag, tag)
			if index%3 == 2 || index == sequence%15-1 {
				input.WriteString("</div>")
			}
		}
		input.WriteString("</article>")
		for variant := 0; variant < 9; variant++ {
			parsed, err := dom.Parse(strings.NewReader(input.String()))
			if err != nil {
				panic(err)
			}
			builder := webdoc.NewWebDocumentBuilder(stringutil.FastWordCounter{}, nil)
			converter.NewDomConverter(converter.Default, builder, nil, nil).Convert(dom.DocumentElement(parsed))
			document := builder.Build().CreateTextDocument()
			for _, block := range document.TextBlocks {
				block.SetIsContent(random.Intn(3) == 0)
				if random.Intn(8) == 0 {
					block.AddLabels(label.StrictlyNotContent)
				}
				if random.Intn(8) == 0 {
					block.AddLabels(label.Title)
				}
			}
			if sequence%2 == 0 && len(document.TextBlocks) > 2 {
				document.TextBlocks[0].MergeNext(document.TextBlocks[1])
				document.TextBlocks = append(document.TextBlocks[:1], document.TextBlocks[2:]...)
			}
			current := siblingCase{HTML: input.String(), Input: encodeBlocks(document), Largest: variant == 8, CrossTitles: variant&1 != 0, CrossHeadings: variant&2 != 0, MixedTags: variant&4 != 0, MaxDensity: []float64{0, 0.2, 0.5}[sequence%3], MaxDistance: []int{0, 1, 2, 10}[sequence%4]}
			if current.Largest {
				current.Changed = heuristic.NewKeepLargestBlock(true).Process(document)
			} else {
				current.Changed = (&heuristic.SimilarSiblingContent{AllowCrossTitles: current.CrossTitles, AllowCrossHeadings: current.CrossHeadings, AllowMixedTags: current.MixedTags, MaxLinkDensity: current.MaxDensity, MaxBlockDistance: current.MaxDistance}).Process(document)
			}
			current.Output = encodeBlocks(document)
			result = append(result, current)
		}
	}
	return result
}

func filterCases() []filterCase {
	names := []string{"heading", "proximity-pre", "proximity-post", "largest", "expand-title", "large-level", "list", "boilerplate-title", "boilerplate-empty", "label", "terminating", "title"}
	labels := []string{label.Title, label.Heading, label.StrictlyNotContent, label.VeryLikelyContent, label.MightBeContent, label.Li, label.BoilerplateHeadingFused}
	texts := []string{"", "Small heading", "The full article title - Example.com", "The full article title", "first part with more words | second part with a substantially longer name", "Comment", "Comments", "Shares", "10 comments", "\u00a9 Reuters", "Please rate this", "This is what you think...", "reader views", "r\u00e4tta artikeln", "Thanks for your comments - this feedback is now closed", "A title!", "A title", "one\u00a0two", "\u0130 title", "\u039f\u03a3"}
	titles := []string{"The full article title - Example.com", "A title", "first part with more words | second part with a substantially longer name", "\u0130 title", "\u039f\u03a3"}
	random := rand.New(rand.NewSource(20260914))
	result := []filterCase{}
	for sequence := 0; sequence < 240; sequence++ {
		input := []blockSnapshot{}
		for index := 0; index < sequence%9; index++ {
			block := blockSnapshot{Text: texts[random.Intn(len(texts))], Labels: []string{}, Words: []int{0, 1, 4, 14, 15, 16, 17, 40, 41, 99, 100, 101}[random.Intn(12)], Level: random.Intn(6) - 1, Content: random.Intn(2) == 0, Elements: []int{index * 2}}
			if block.Words > 0 {
				block.AnchorWords = random.Intn(block.Words + 1)
				block.Density = float64(block.AnchorWords) / float64(block.Words)
			}
			for _, name := range labels {
				if random.Intn(5) == 0 {
					block.Labels = append(block.Labels, name)
				}
			}
			sort.Strings(block.Labels)
			input = append(input, block)
		}
		for _, name := range names {
			var current filter.TextDocumentFilter
			switch name {
			case "heading":
				current = heuristic.NewHeadingFusion()
			case "proximity-pre":
				current = heuristic.NewBlockProximityFusion(false)
			case "proximity-post":
				current = heuristic.NewBlockProximityFusion(true)
			case "largest":
				current = heuristic.NewKeepLargestBlock(false)
			case "expand-title":
				current = heuristic.NewExpandTitleToContent()
			case "large-level":
				current = heuristic.NewLargeBlockAroundTagLevelToContent()
			case "list":
				current = heuristic.NewListAtEnd()
			case "boilerplate-title":
				current = simple.NewBoilerplateBlock(label.Title)
			case "boilerplate-empty":
				current = simple.NewBoilerplateBlock("")
			case "label":
				current = simple.NewLabelToBoilerplate(label.StrictlyNotContent)
			case "terminating":
				current = english.NewTerminatingBlocksFinder()
			case "title":
				current = heuristic.NewDocumentTitleMatch(stringutil.FastWordCounter{}, titles...)
			}
			document := decodeBlocks(input)
			changed := current.Process(document)
			result = append(result, filterCase{name, input, encodeBlocks(document), titles, changed})
		}
	}
	return result
}
