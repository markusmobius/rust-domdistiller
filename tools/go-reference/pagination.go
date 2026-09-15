package main

import (
	"fmt"
	"math/rand"
	"net/url"
	"path"
	"sort"
	"strings"

	"github.com/go-shiori/dom"
	"github.com/markusmobius/go-domdistiller/data"
	"github.com/markusmobius/go-domdistiller/internal/pagination"
	"github.com/markusmobius/go-domdistiller/internal/pagination/info"
	"github.com/markusmobius/go-domdistiller/internal/pagination/pattern"
	"github.com/markusmobius/go-domdistiller/internal/stringutil"
)

type paginationCase struct {
	HTML     string              `json:"html"`
	URL      string              `json:"url"`
	PrevNext data.PaginationInfo `json:"prev_next"`
	Number   data.PaginationInfo `json:"number"`
}

func paginationCases() []paginationCase {
	cases := []paginationCase{}
	for _, pageURL := range []string{"https://example.com/story/page/2/", "https://example.com/story.html?p=2", "https://example.com/2020/story2", "https://example.com/story/", "https://example.com/"} {
		for _, parent := range []string{"", "pagination", "footer", "body-footer", "sidebar pagination"} {
			for _, text := range []string{"Next", "Previous", "next page", "continue", "weiter", ">", "&raquo;", ">|", "1", "2", "3", "first", "last", "all pages", "a very long link description", "newer", "older"} {
				for _, extra := range []string{"", "<a href='/story/page/3/'>print</a>", "<a class='next pagination' href='/story/page/4/'>4</a>"} {
					input := fmt.Sprintf("<div class='%s'><a href='/story/page/1/'>Previous</a><a href='/story/page/2/'>2</a><a href='/story/page/3/#section'>%s</a>%s</div>", parent, text, extra)
					parsed, err := dom.Parse(strings.NewReader(input))
					if err != nil {
						panic(err)
					}
					page, err := url.Parse(pageURL)
					if err != nil {
						panic(err)
					}
					info := pagination.NewPrevNextFinder(nil).FindPagination(dom.DocumentElement(parsed), page)
					number := pagination.NewPageNumberFinder(stringutil.SelectWordCounter(dom.TextContent(parsed)), nil, nil).FindPagination(dom.DocumentElement(parsed), page)
					cases = append(cases, paginationCase{input, pageURL, info, number})
				}
			}
		}
	}
	for _, path := range []string{"/story?page=%d", "/story/page/%d", "/story-%d.html", "/2020/01/%d", "/story?offset=%d", "/story?tag=%d"} {
		for _, current := range []int{1, 2, 3, 4, 5} {
			for _, size := range []int{2, 3, 5, 10, 28, 32} {
				for _, mode := range []int{0, 1, 2, 3} {
					var input strings.Builder
					input.WriteString("<div class='pages'>")
					for index := 1; index <= size; index++ {
						number := index
						if mode == 3 {
							number = size + 1 - index
						}
						if number == current && mode != 2 {
							input.WriteString(fmt.Sprintf("<span>(%d)</span> ", number))
						} else {
							href := fmt.Sprintf(path, number)
							if mode == 1 && number == 1 {
								href = "/story"
							}
							input.WriteString(fmt.Sprintf("<a href='%s'>[%d]</a> ", href, number))
						}
					}
					input.WriteString("</div>")
					pageURL := "https://example.com" + fmt.Sprintf(path, current)
					if mode == 1 && current == 1 {
						pageURL = "https://example.com/story"
					}
					parsed, err := dom.Parse(strings.NewReader(input.String()))
					if err != nil {
						panic(err)
					}
					page, err := url.Parse(pageURL)
					if err != nil {
						panic(err)
					}
					prevNext := pagination.NewPrevNextFinder(nil).FindPagination(dom.DocumentElement(parsed), page)
					number := pagination.NewPageNumberFinder(stringutil.SelectWordCounter(dom.TextContent(parsed)), nil, nil).FindPagination(dom.DocumentElement(parsed), page)
					cases = append(cases, paginationCase{input.String(), pageURL, prevNext, number})
				}
			}
		}
	}
	for _, pagePath := range []string{
		"/news/story2", "/news//story2", "/news///story2", "/news/./story2",
		"/news/archive/../story2", "/news/../../story2", "//story2", "/./story2",
		"/web/20190717140047/http://example.com/story2", "/news/%2F/story2",
		"/news/%2e%2e/story2", "/news/story2/",
	} {
		pageURL := "https://example.com" + pagePath
		page, err := url.Parse(pageURL)
		if err != nil {
			panic(err)
		}
		unclean := strings.TrimSuffix(page.Path, "/")
		unclean = unclean[:strings.LastIndex(unclean, "/")+1]
		clean := strings.TrimSuffix(path.Dir(strings.TrimSuffix(page.Path, "/")), "/")
		for _, folder := range []string{strings.TrimSuffix(unclean, "/"), clean} {
			for _, parent := range []string{"", "pagination", "footer"} {
				for _, text := range []string{"Previous", "Next"} {
					input := fmt.Sprintf("<div class='%s'><a href='https://example.com%s/page/3'>%s</a></div>", parent, folder, text)
					parsed, err := dom.Parse(strings.NewReader(input))
					if err != nil {
						panic(err)
					}
					root := dom.DocumentElement(parsed)
					prevNext := pagination.NewPrevNextFinder(nil).FindPagination(root, page)
					number := pagination.NewPageNumberFinder(stringutil.SelectWordCounter(dom.TextContent(parsed)), nil, nil).FindPagination(root, page)
					cases = append(cases, paginationCase{input, pageURL, prevNext, number})
				}
			}
		}
	}
	return cases
}

type pageInfoSnapshot struct {
	Number int    `json:"number"`
	URL    string `json:"url"`
}

type pageGroupSnapshot struct {
	Pages []pageInfoSnapshot `json:"pages"`
	Delta int                `json:"delta"`
}

type numberGroupsCase struct {
	Input  []*int              `json:"input"`
	Groups []pageGroupSnapshot `json:"groups"`
}

func numberGroupsCases() []numberGroupsCase {
	random := rand.New(rand.NewSource(20260916))
	cases := []numberGroupsCase{}
	for sequence := 0; sequence < 240; sequence++ {
		groups := &info.MonotonicPageInfoGroups{}
		input := []*int{}
		for index := 0; index < 40; index++ {
			if index == 0 || random.Intn(7) == 0 {
				groups.AddGroup()
				input = append(input, nil)
			} else {
				number := random.Intn(12)
				input = append(input, &number)
				groups.AddNumber(number, fmt.Sprintf("page%d", number))
			}
		}
		groups.CleanUp()
		snapshots := []pageGroupSnapshot{}
		for _, group := range groups.Groups {
			pages := []pageInfoSnapshot{}
			for _, page := range group.List {
				pages = append(pages, pageInfoSnapshot{page.PageNumber, page.URL})
			}
			snapshots = append(snapshots, pageGroupSnapshot{pages, group.DeltaSign})
		}
		cases = append(cases, numberGroupsCase{input, snapshots})
	}
	return cases
}

type pagePatternSnapshot struct {
	Kind   string `json:"kind"`
	Value  string `json:"value"`
	Number int    `json:"number"`
	Valid  []bool `json:"valid"`
	Paging []bool `json:"paging"`
}

type pagePatternCase struct {
	URL       string                `json:"url"`
	Documents []string              `json:"documents"`
	Patterns  []pagePatternSnapshot `json:"patterns"`
}

func pagePatternCases() []pagePatternCase {
	paths := []string{"/story", "/story/", "/story/1", "/story/page/2", "/story/page/3/", "/story-2.html", "/story_3.htm", "/1", "/2.html", "/category/2", "/tag/2/other", "/2020/01/2", "/story/2020/01/headline_Page2.html", "/a/2/tail.html", "/a/12/2", "/a/2?fixed=x", "/story.html?p=2", "/story.html?p=2&other=constant", "/story.html?page=01&page=2", "/story.html?p=2&q=7", "/story.html?Page=3&x=4", "/story.html?category=3", "/story.html?p=-2", "/story.html?p=+2", "/story.html?p=9999999999999999999999", "/story.html?p=2&fixed=a+b~c*", "/story.html?offset=20", "/story.html?=2", "/a/b-03.html", "/a/b;2.html", "/a/b,2.html", "/a/b2.html", "/a/b2shtml", "/a/b/2.html?x=y", "/a/2/3.html", "/a/2/tail", "/a/2/other/3", "/story.HTML?page=2", "/story.html?page=2&sortby=4", "/story.html?page=2&empty="}
	documents := []string{}
	for _, path := range []string{"/story", "/story/", "/story.html", "/story.html?p=1", "/story.html?p=2&other=constant", "/story.html?page=3", "/story.html?page=+3", "/story.html?page=no", "/story.html?page=3&extra=1", "/story-1.html", "/story/page/2", "/story/page/2/", "/a/b.html", "/a/b-1.html", "/a/b;1.html", "/a/b,1.html", "/a/1/tail.html", "/a/tail.html", "/tail.html", "/a/12", "/2020/01/1", "/a/2/tail", "/a/2/other/2"} {
		documents = append(documents, "https://example.com"+path)
	}
	documents = append(documents, "http://example.com/story.html?p=2", "https://other.test/story.html?p=2")
	cases := []pagePatternCase{}
	for _, path := range paths {
		pageURL := "https://example.com" + path
		parsed, err := url.Parse(pageURL)
		if err != nil {
			panic(err)
		}
		snapshots := []pagePatternSnapshot{}
		for _, kind := range []string{"query", "path"} {
			patterns := pattern.QueryParamPagePatternsFromURL(parsed)
			if kind == "path" {
				patterns = pattern.PathComponentPagePatternsFromURL(parsed)
			}
			for _, candidate := range patterns {
				valid, paging := []bool{}, []bool{}
				for _, document := range documents {
					parsed, err := url.Parse(document)
					if err != nil {
						panic(err)
					}
					valid = append(valid, candidate.IsValidFor(parsed))
					paging = append(paging, candidate.IsPagingURL(document))
				}
				snapshots = append(snapshots, pagePatternSnapshot{kind, candidate.String(), candidate.PageNumber(), valid, paging})
			}
		}
		sort.Slice(snapshots, func(left, right int) bool {
			if snapshots[left].Kind != snapshots[right].Kind {
				return snapshots[left].Kind < snapshots[right].Kind
			}
			if snapshots[left].Value != snapshots[right].Value {
				return snapshots[left].Value < snapshots[right].Value
			}
			return snapshots[left].Number < snapshots[right].Number
		})
		cases = append(cases, pagePatternCase{pageURL, documents, snapshots})
	}
	return cases
}
