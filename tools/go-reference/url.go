package main

import (
	"net/url"

	"github.com/markusmobius/go-domdistiller/internal/stringutil"
)

type urlCase struct {
	Base     *string `json:"base"`
	Input    string  `json:"input"`
	Absolute string  `json:"absolute"`
}

func urlCases() []urlCase {
	inputs := []string{"", "#here", "/test/123", "test/123", "//www.google.com", "https://www.google.com", "ftp://ftp.server.com", "www.google.com", "http//www.google.com", "../hello/relative", "image.png", "?page=2", "?", "./", "..", "../../", "//EXAMPLE.COM:443", "//example.com:80/path", "//user:pass@Example.COM", "/a/../b", "/a/%2e%2e/b", "/a%2fb", "/a//b", "a b.jpg", " a.jpg ", "a\\b", "a\tb", "a\nb", "%zz", "//example.com:bad/path", "http:relative", "mailto:person@example.com", "data:image/png;base64,AA==", "javascript:void(0)", "DATA:image/png;base64,AA==", "http://EXAMPLE.com:80", "HTTP://EXAMPLE.com:80", "///triple/path", "//[::1]:8080/a", "/caf\u00e9.jpg", "//ex\u00e4mple.com/photo", "?q=a+b&q=second", "./a:b", "a:b", "https://example.com/%zz", "//example.com/%zz", "//example.com?x=1", "/a#hello world", "image.png#", "/a?", "../x?raw=%zz", "//example.com/a?bad=%zz", "\\\\example.com\\a", "https://example.com/a b"}
	cases := []urlCase{}
	bases := []*string{nil}
	for _, value := range []string{"", "http://example.com/page/", "http://example.com/page/doc.html", "https://EXAMPLE.com:443/a%2fb/file?old=1#section", "https://example.com", "https://example.com/a//b/../c/", "/relative/base", "mailto:person@example.com"} {
		value := value
		bases = append(bases, &value)
	}
	for _, base := range bases {
		var parsed *url.URL
		if base != nil {
			var err error
			parsed, err = url.Parse(*base)
			if err != nil {
				panic(err)
			}
		}
		for _, input := range inputs {
			cases = append(cases, urlCase{base, input, stringutil.CreateAbsoluteURL(input, parsed)})
		}
	}
	return cases
}
