package main

import (
	"strings"

	"github.com/go-shiori/dom"
	"github.com/markusmobius/go-domdistiller/data"
	"github.com/markusmobius/go-domdistiller/internal/extractor"
	"github.com/markusmobius/go-domdistiller/internal/markup"
)

type markupCase struct {
	HTML        string          `json:"html"`
	Info        data.MarkupInfo `json:"info"`
	Title       string          `json:"title"`
	MarkupTitle string          `json:"markup_title"`
}

func markupCases() []markupCase {
	inputs := []string{"", "<title>Short</title><h1>A longer heading for this article</h1>", "<title>Site: A long article with several words</title>", "<title>A long article with several words - Site</title>", "<title>Site | A long article with several words</title>", "<title>Section / A longer title</title>", "<title>Topic: Actual title</title><h2>Topic: Actual title</h2>"}
	for _, prefix := range []string{"", "prefix='social: http://ogp.me/ns# person: http://ogp.me/ns/profile# story: http://ogp.me/ns/article#'", "xmlns:social='http://ogp.me/ns#' xmlns:person='http://ogp.me/ns/profile#' xmlns:story='http://ogp.me/ns/article#'"} {
		og, profile, article := "og", "profile", "article"
		if prefix != "" {
			og, profile, article = "social", "person", "story"
		}
		for _, kind := range []string{"article", "profile", "website", "ARTICLE"} {
			for omit := -1; omit < 4; omit++ {
				for _, early := range []bool{false, true} {
					var input strings.Builder
					input.WriteString("<html " + prefix + "><head><title>Document title | Publisher</title><meta name='title' content='IE title'><meta name='displaydate' content='2020-01-02'><meta name='copyright' content='Copyright holder'>")
					properties := []string{"<meta property='" + article + ":published_time' content='2021-03-04'>", "<meta property='" + article + ":author' content='first-author'>", "<meta property='" + profile + ":first_name' content='First'>"}
					if early {
						for _, property := range properties {
							input.WriteString(property)
						}
					}
					for index, property := range []string{"<meta property='" + og + ":title' content='OG title'>", "<meta property='" + og + ":type' content='" + kind + "'>", "<meta property='" + og + ":url' content='https://example.com/story'>", "<meta property='" + og + ":image' content='cover.jpg'>"} {
						if index != omit {
							input.WriteString(property)
						}
					}
					if !early {
						for _, property := range properties {
							input.WriteString(property)
						}
					}
					input.WriteString("<meta property='" + profile + ":last_name' content='Last'><meta property='" + article + ":modified_time' content='2021-04-05'><meta property='" + article + ":author' content='second-author'><meta property='" + og + ":image:width' content='600'><meta property='" + og + ":image:height' content='400'><meta property='" + og + ":description' content='Description'><meta property='" + og + ":site_name' content='Publisher'></head><body><div class='byline-name'> Writer </div><div class='dateline'>IE date</div><p publisher='IE publisher'>body</p></body></html>")
					inputs = append(inputs, input.String())
				}
			}
		}
	}
	for _, extra := range []string{"<meta name='IE_RM_OFF' content='true'>", "<meta name='IE_RM_OFF' content='false'>", "<meta name='IE_RM_OFF' content='true'><meta name='IE_RM_OFF' content='false'>", "<meta name='IE_RM_OFF' content='false'><meta name='IE_RM_OFF' content='true'>"} {
		inputs = append(inputs, "<title>Document title</title><meta name='title' content='Metadata title'>"+extra+"<h1>Article heading</h1>")
	}
	for _, width := range []string{"399", "400", "600", "-1", "400px", "9999999999999999999999999"} {
		for _, height := range []string{"0", "100", "200", "300", "-1"} {
			for _, caption := range []string{"", "<figcaption>Caption <b>text</b>.</figcaption>", "<figcaption hidden>hidden</figcaption><figcaption>visible</figcaption>", "<figcaption>a</figcaption><figcaption>b</figcaption><figcaption>c</figcaption>"} {
				inputs = append(inputs, "<figure><img src='photo.jpg' width='"+width+"' height='"+height+"'>"+caption+"</figure>")
			}
		}
	}
	for _, scheme := range []string{"http", "https"} {
		for _, kind := range []string{"Article", "BlogPosting", "NewsArticle", "ScholarlyArticle", "TechArticle", "Unknown"} {
			for _, nested := range []string{"", "<div itemscope itemtype='http://schema.org/Unsupported'>"} {
				for _, author := range []string{"<span itemprop='author'>Author text</span>", "<span itemprop='author' itemscope itemtype='http://schema.org/Person'><span itemprop='givenName'>Given</span><span itemprop='familyName'>Family</span></span>", "<span itemprop='creator' itemscope itemtype='http://schema.org/Organization'><span itemprop='legalName'>Organization</span></span>"} {
					input := nested + "<article itemscope itemtype='" + scheme + "://schema.org/" + kind + "'><h1 itemprop='headline name'>  Schema title  </h1><h2 itemprop='headline'>Second title</h2><meta itemprop='url' content='relative/story'><span itemprop='description'>Description</span><time itemprop='datePublished' datetime='2020-01-02'>Other date</time><meta itemprop='dateModified' content='2020-02-03'>" + author + "<span itemprop='publisher' itemscope itemtype='http://schema.org/Corporation'><span itemprop='legalName'>Publisher</span></span><span itemprop='copyrightHolder'>Holder</span><span itemprop='copyrightYear'>2020</span><meta itemprop='articleSection' content='News'></article><a rel='author'>Rel author</a>"
					inputs = append(inputs, input)
				}
			}
		}
	}
	for _, associated := range []string{"associatedMedia", "encoding", "image", "unknown"} {
		for _, representative := range []string{"true", "false"} {
			inputs = append(inputs, "<div itemscope itemtype='http://schema.org/ImageObject'><meta itemprop='contentUrl' content='first.jpg'><meta itemprop='representativeOfPage' content='"+representative+"'></div><article itemscope itemtype='http://schema.org/Article'><h1 itemprop='headline'>First article</h1><img itemprop='image' src='inline.jpg'><div itemprop='"+associated+"' itemscope itemtype='http://schema.org/ImageObject'><meta itemprop='contentUrl' content='associated.jpg'><meta itemprop='width' content='800'><meta itemprop='height' content='450'><meta itemprop='encodingFormat' content='image/jpeg'><span itemprop='caption'>Caption</span></div></article><article itemscope itemtype='http://schema.org/Article'><img itemprop='image' src='second.jpg'></article><div itemscope itemtype='http://schema.org/ImageObject'><a itemprop='url' href='last.jpg'></a></div>")
		}
	}
	inputs = append(inputs,
		"<html itemscope itemtype='http://schema.org/Article'><head><meta itemprop='headline' content='Root title'></head><body><span itemprop='author'>Root author</span></body></html>",
		"<article itemscope itemtype='http://schema.org/Article'><h1 itemprop='headline'> </h1><h2 itemprop='headline'>Nonempty title</h2><div itemprop='author' itemscope itemtype='http://schema.org/Unknown'><span itemprop='name'>Ignore</span></div><span itemprop='creator'>Creator</span></article>",
		"<article itemscope itemtype='http://schema.org/Article'><div itemprop='author' itemscope itemtype='http://schema.org/Person'><span itemprop='familyName'>Family only</span></div><a rel='author'>Fallback</a></article>",
		"<a rel='AUTHOR'>Upper</a><a rel='author other'>Multiple</a><a rel='author'>Expected</a>",
	)
	cases := []markupCase{}
	for _, input := range inputs {
		parsed, err := dom.Parse(strings.NewReader(input))
		if err != nil {
			panic(err)
		}
		root := dom.DocumentElement(parsed)
		parser := markup.NewParser(root, &data.TimingInfo{})
		cases = append(cases, markupCase{input, parser.MarkupInfo(), extractor.NewContentExtractor(root, nil, nil).ExtractTitle(), parser.Title()})
	}
	return cases
}
