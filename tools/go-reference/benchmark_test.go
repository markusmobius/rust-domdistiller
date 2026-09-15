package main

import "testing"

func TestBenchmarkSnippetScoring(t *testing.T) {
	page := benchmarkPage{With: []string{"article", "missing", "article"}, Without: []string{"navigation", "footer"}}
	var counts benchmarkCounts
	counts.evaluate("article navigation", page)
	if counts != (benchmarkCounts{2, 1, 1, 1}) {
		t.Fatalf("unexpected counts: %+v", counts)
	}
	counts.evaluate("", page)
	if counts != (benchmarkCounts{2, 4, 1, 3}) {
		t.Fatalf("unexpected empty-result counts: %+v", counts)
	}
}
