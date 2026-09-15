package main

import (
	"net/url"
	"strings"

	"github.com/go-shiori/dom"
	"github.com/markusmobius/go-domdistiller/internal/converter"
	"github.com/markusmobius/go-domdistiller/internal/domutil"
	"github.com/markusmobius/go-domdistiller/internal/extractor"
	"github.com/markusmobius/go-domdistiller/internal/filter/docfilter"
	"github.com/markusmobius/go-domdistiller/internal/stringutil"
	"github.com/markusmobius/go-domdistiller/internal/tableclass"
	"github.com/markusmobius/go-domdistiller/internal/webdoc"
	"sort"
)

type domNode struct {
	Tag     string `json:"tag"`
	Display string `json:"display"`
	Visible bool   `json:"visible"`
}

type domCase struct {
	Input     string    `json:"input"`
	HTML      string    `json:"html"`
	Text      string    `json:"text"`
	InnerText string    `json:"inner_text"`
	Nodes     []domNode `json:"nodes"`
}

func domCases() []domCase {
	inputs := []string{
		"", "text", "<!DOCTYPE html><title>A &amp; B</title><p>Hello <b>world</b> ! <br> Next.",
		"<p id='a'>before<span hidden>hidden</span>after<br hidden>last",
		"<a z='2' href='/x?y=1&amp;z=2' id='a'>link</a>",
		"<p a='1' a='2' b='3'>text</p>",
		"<pre>\n\n raw &lt;text&gt; </pre><textarea>\n\nnext</textarea>",
		"<noscript><p>fallback</p></noscript><p>visible</p>",
		"<template><p>template</p></template><p>outside</p>",
		"<svg><a xlink:href='/x'>link</a><foreignObject><p>HTML</p></foreignObject></svg>",
		"<table>before<tr><td>cell</td></tr>after</table>",
		"<p>one<b>two<i>three</b>four</i>five",
		"<!--before--><!DOCTYPE HTML PUBLIC 'public' 'system'><p>&apos; &quot; &#13; &nbsp;</p>",
		"<p>A<span> , </span>B<br> C <br><br> D</p>",
		"<div style='display:none'>hidden</div><div style='display:NONE'>shown</div>",
		"<div aria-hidden='true'>hidden</div><div aria-hidden='true' class='fallback-image'>shown</div>",
	}
	for _, style := range []string{"", "display: none;", "display :none", "DISPLAY: inline-flex", "display:none!important", "visibility: hidden", "visibility::hidden", "visibility:collapse", "display:\u00a0none", "display:\vnone", "display:block;display:none"} {
		inputs = append(inputs, "<div style='"+style+"'>before<span>inside</span>after</div>")
	}
	for _, tag := range []string{"div", "aside", "unknown", "a", "span", "li", "summary", "ruby", "rt", "audio", "video", "button", "input", "hr"} {
		inputs = append(inputs, "<"+tag+">content</"+tag+">")
	}
	cases := []domCase{}
	for _, input := range inputs {
		parsed, err := dom.Parse(strings.NewReader(input))
		if err != nil {
			panic(err)
		}
		parsed = dom.Clone(parsed, true)
		current := domCase{Input: input, HTML: dom.OuterHTML(parsed), Text: dom.TextContent(parsed), InnerText: domutil.InnerText(parsed), Nodes: []domNode{}}
		for _, node := range dom.GetElementsByTagName(parsed, "*") {
			current.Nodes = append(current.Nodes, domNode{dom.TagName(node), domutil.GetDisplayStyle(node), domutil.IsProbablyVisible(node)})
		}
		cases = append(cases, current)
	}
	return cases
}

type elementSnapshot struct {
	Kind        string   `json:"kind"`
	Text        string   `json:"text"`
	Labels      []string `json:"labels"`
	Words       int      `json:"words"`
	AnchorWords int      `json:"anchor_words"`
	Level       int      `json:"level"`
	Group       int      `json:"group"`
	Tag         string   `json:"tag"`
	Start       bool     `json:"start"`
}

type converterCase struct {
	Input    string            `json:"input"`
	Skip     bool              `json:"skip"`
	Elements []elementSnapshot `json:"elements"`
	Blocks   []blockSnapshot   `json:"blocks"`
	Article  []blockSnapshot   `json:"article"`
	HTML     string            `json:"html"`
	Text     string            `json:"text"`
	Content  []bool            `json:"content"`
	Images   []string          `json:"images"`
}

func converterCases() []converterCase {
	inputs := []string{
		"", "visible text", "<div>visible parent<div>visible child</div></div>",
		"<p>The <a href='/a'>Overview</a> is here.</p>",
		"<p>text<br>split<br/>with<br/>lines</p>",
		"<ol><li>some text1</li><li>some text2</li></ol>",
		"<blockquote>quoted <p>paragraph</p> after</blockquote>",
		"<ul><li>first<p>paragraph <b>bold</b></p><ul><li>nested</li></ul>end</li></ul>",
		"<p>Before<input value='skip'>After</p><p>last</p>",
		"<p>Before<a href='javascript:go()'>text</a>After</p><p>last</p>",
		"<p>Before<a href='javascript:go()'><b>text</b></a>After</p>",
		"<div class='sharing'>share</div><div data-component='share'>share</div><p>body</p>",
		"<div class='author'>by author</div><div rel='author'>writer</div><p>body</p>",
		"<header>site header</header><div class='header'>likely header</div><div class='article-header'>candidate</div>",
		"<nav>nav text</nav><aside>side text</aside><div id='comments'>comment text</div>",
		"<div class='comments one two'>three classes</div><div class='comments one'>two</div>",
		"<h1>Title</h1><h2>Second</h2><h3>Third</h3><h4>Fourth</h4><h5>Fifth</h5><h6>Sixth</h6>",
		"<div hidden>hidden</div><p>A<span style='display:none'>hidden</span>B</p>",
		"<div><br><hr></div><li></li><section></section>",
		"<p><font color='red'>font text</font><small>small text</small></p>",
		"<p><a href='index.php?action=edit&amp;section=3'>edit</a>main<span class='mw-editsection'>[edit]</span></p>",
		"<div role='navigation'>navigation</div><div role='main'>content</div>",
		"<div><span style='display:inline-block'>nested <a>anchor</a></span>after</div>",
		"<summary><p>inside</p>text</summary><pre>  pre\n   text\n</pre>",
		"<p>\u5b57\u5b57 <a href='/x'>\ud55c\uae00</a> \u00e9criture</p>",
		"<p> \t <b> </b>before<a href='/x'> </a>after</p>",
		"<p>" + strings.Repeat("article body words ", 30) + "</p>",
		"<ul><li>" + strings.Repeat("article body words ", 30) + "</li><li>short list item</li></ul>",
		"<blockquote><p>" + strings.Repeat("quoted article words ", 30) + "</p><p>short quote</p></blockquote>",
		"<div>" + strings.Repeat("article body words ", 30) + "<a href='../next'>next link</a> tail.</div>",
		strings.Repeat("body without paragraph ", 30),
		"<p id='removed' title='retained' onclick='unsafe()' data-x='removed'>" + strings.Repeat("article body words ", 30) + "</p>",
		"<img src='lead.jpg'><p>" + strings.Repeat("article body words ", 30) + "</p><img src='last.jpg'>",
		"<figure><img src='lead.jpg'><figcaption>Photo <b>caption</b>.</figcaption></figure><p>" + strings.Repeat("article body words ", 30) + "</p>",
		"<figure><picture><source srcset='a.jpg 1x, b.jpg 2x'><img src='fallback.jpg'></picture><figcaption>Credit <a href='../author'>Author</a></figcaption></figure><p>" + strings.Repeat("article body words ", 30) + "</p>",
		"<p>" + strings.Repeat("article body words ", 30) + "<img data-src='real.jpg' src='data:image/png;base64,AAAA' width='600' onload='unsafe()'>more text</p>",
		"<picture><source data-srcset='one.jpg 1x, two.jpg 2x'><span>removed</span></picture><p>" + strings.Repeat("article body words ", 30) + "</p>",
		"<figure><img src='lazy.jpg'><noscript><img src='real.jpg'></noscript><figcaption>caption</figcaption></figure><p>" + strings.Repeat("article body words ", 30) + "</p>",
		"<span class='lazy-image-placeholder' data-src='wiki.jpg' data-srcset='wiki2.jpg 2x'></span><p>" + strings.Repeat("article body words ", 30) + "</p>",
		"<img srcset='one.jpg 1x, two.jpg 2x'><p>" + strings.Repeat("article body words ", 30) + "</p>",
		"<p>" + strings.Repeat("article body words ", 30) + "</p><table role='grid' id='remove'><tr><td width='100'>cell<img src='cell.jpg' srcset='cell2.jpg 2x'></td></tr></table>",
		"<p>" + strings.Repeat("article body words ", 30) + "</p><table><tr><td>cell1</td><td>cell2</td></tr></table>",
		"<p>" + strings.Repeat("article body words ", 30) + "</p><video src='movie.mp4' poster='poster.jpg' controls width='800'><source src='other.mp4'><track src='captions.vtt'><p>fallback</p>text</video>",
		"<p>" + strings.Repeat("article body words ", 30) + "</p><blockquote class='twitter-tweet'><p>Tweet body</p><a href='https://twitter.com/person/status/123/'>date</a></blockquote>",
		"<p>" + strings.Repeat("article body words ", 30) + "</p><iframe src='https://platform.twitter.com/embed' data-tweet-id='456'></iframe>",
		"<p>" + strings.Repeat("article body words ", 30) + "</p><iframe src='//player.vimeo.com/video/123?color=red'></iframe>",
		"<p>" + strings.Repeat("article body words ", 30) + "</p><iframe src='https://www.youtube.com/embed/AbCd?start=10' width='800' allowfullscreen></iframe>",
		"<p>" + strings.Repeat("article body words ", 30) + "</p><object type='application/x-shockwave-flash' data='http://youtube.com/v/AbCd&amp;start=10'></object>",
		"<p>" + strings.Repeat("article body words ", 30) + "</p><object><param name='movie' value='https://www.youtube-nocookie.com/embed/AbCd'></object><iframe src='https://unrelated.test/embed'></iframe>",
	}
	cases := []converterCase{}
	for _, input := range inputs {
		for _, skip := range []bool{false, true} {
			parsed, err := dom.Parse(strings.NewReader(input))
			if err != nil {
				panic(err)
			}
			root := dom.DocumentElement(parsed)
			pageURL, _ := url.Parse("https://example.com/news/story.html")
			builder := webdoc.NewWebDocumentBuilder(stringutil.SelectWordCounter(dom.TextContent(parsed)), pageURL)
			flags := converter.Default
			if skip {
				flags = converter.SkipUnlikelies
			}
			converter.NewDomConverter(flags, builder, pageURL, nil).Convert(root)
			document := builder.Build()
			current := converterCase{Input: input, Skip: skip, Elements: []elementSnapshot{}, Blocks: []blockSnapshot{}}
			for _, element := range document.Elements {
				snapshot := elementSnapshot{Kind: element.ElementType(), Labels: []string{}}
				switch item := element.(type) {
				case *webdoc.Text:
					snapshot.Text = item.Text
					snapshot.Words = item.NumWords
					snapshot.AnchorWords = item.NumLinkedWords
					snapshot.Level = item.TagLevel
					snapshot.Group = item.GroupNumber
					for value := range item.Labels {
						snapshot.Labels = append(snapshot.Labels, value)
					}
					sort.Strings(snapshot.Labels)
				case *webdoc.Tag:
					snapshot.Tag = item.Name
					snapshot.Start = item.Type == webdoc.TagStart
				case *webdoc.Image, *webdoc.Figure, *webdoc.Table, *webdoc.Video, *webdoc.Embed:
				default:
					panic("unexpected nontext element")
				}
				current.Elements = append(current.Elements, snapshot)
			}
			textDocument := document.CreateTextDocument()
			current.Blocks = encodeBlocks(textDocument)
			extractor.NewArticleExtractor(nil).Extract(textDocument, stringutil.SelectWordCounter(dom.TextContent(parsed)), nil)
			current.Article = encodeBlocks(textDocument)
			textDocument.ApplyToModel()
			docfilter.NewRelevantElements().Process(document)
			docfilter.NewLeadImageFinder(nil).Process(document)
			docfilter.NewNestedElementRetainer().Process(document)
			current.HTML = document.GenerateOutput(false)
			current.Text = document.GenerateOutput(true)
			current.Images = document.GetImageURLs()
			current.Content = []bool{}
			for _, element := range document.Elements {
				current.Content = append(current.Content, element.IsContent())
			}
			cases = append(cases, current)
		}
	}
	return cases
}

type tableCase struct {
	HTML   string `json:"html"`
	Data   bool   `json:"data"`
	Reason string `json:"reason"`
}

func tableCases() []tableCase {
	cases := []tableCase{}
	for _, rows := range []int{0, 1, 2, 3, 6, 19, 20} {
		for _, columns := range []int{0, 1, 2, 4, 5} {
			for _, attribute := range []string{"", "role='presentation'", "role='grid'", "role='main'", "datatable='0'", "summary=''"} {
				for variant := 0; variant < 10; variant++ {
					var input strings.Builder
					if variant == 1 {
						input.WriteString("<div contenteditable='TRUE'>")
					}
					input.WriteString("<table " + attribute + ">")
					if variant == 2 {
						input.WriteString("<caption>Caption</caption>")
					}
					if variant == 3 {
						input.WriteString("<thead></thead>")
					}
					for row := 0; row < rows; row++ {
						input.WriteString("<tr>")
						for column := 0; column < columns; column++ {
							if variant == 4 {
								input.WriteString("<td scope='col'>")
							} else if variant == 5 {
								input.WriteString("<td colspan='3'>")
							} else {
								input.WriteString("<td>")
							}
							switch variant {
							case 6:
								input.WriteString("<abbr>value</abbr>")
							case 7:
								input.WriteString("<table><tr><td>nested</td></tr></table>")
							case 8:
								input.WriteString("<iframe></iframe>")
							case 9:
								input.WriteString("<span role='rowheader'>value</span>")
							default:
								input.WriteString("value")
							}
							input.WriteString("</td>")
						}
						input.WriteString("</tr>")
					}
					input.WriteString("</table>")
					parsed, err := dom.Parse(strings.NewReader(input.String()))
					if err != nil {
						panic(err)
					}
					kind, reason := tableclass.NewClassifier(nil).Classify(dom.QuerySelector(parsed, "table"))
					cases = append(cases, tableCase{input.String(), kind == tableclass.Data, reason.String()})
				}
			}
		}
	}
	return cases
}
