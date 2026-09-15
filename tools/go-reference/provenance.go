package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"time"
)

type sourceFile struct {
	Source      string `json:"source"`
	Destination string `json:"destination,omitempty"`
	SHA256      string `json:"sha256"`
	Bytes       int    `json:"bytes"`
}

func exportNotices(root string) {
	modules := map[string]string{}
	for name, version := range dependencyVersions {
		directory, err := exec.Command(filepath.Join(runtime.GOROOT(), "bin", "go"), "list", "-mod=readonly", "-m", "-f", "{{.Dir}}", name).Output()
		if err != nil {
			panic(err)
		}
		modules[name+"@"+version] = strings.TrimSpace(string(directory))
	}
	distiller := "github.com/markusmobius/go-domdistiller@" + moduleVersion
	shiori := "github.com/go-shiori/dom@" + dependencyVersions["github.com/go-shiori/dom"]
	chardet := "github.com/gogs/chardet@" + dependencyVersions["github.com/gogs/chardet"]
	text := "golang.org/x/text@" + dependencyVersions["golang.org/x/text"]
	network := "golang.org/x/net@" + dependencyVersions["golang.org/x/net"]
	files := []struct{ module, source, destination string }{
		{distiller, "LICENSE", "LICENSE"},
		{distiller, "LICENSE-domdistiller.txt", "licenses/LICENSE-domdistiller-upstream.txt"},
		{distiller, "LICENSE-boilerpipe.txt", "licenses/LICENSE-boilerpipe.txt"},
		{distiller, "NOTICE-boilerpipe.txt", "licenses/NOTICE-boilerpipe.txt"},
		{shiori, "LICENSE", "licenses/LICENSE-shiori.txt"},
		{chardet, "LICENSE", "licenses/LICENSE-chardet.txt"},
		{chardet, "icu-license.html", "licenses/icu-license.html"},
		{text, "LICENSE", "licenses/LICENSE-go.txt"},
		{network, "LICENSE", ""},
		{distiller, "example/sample.html", ""},
		{chardet, "detector.go", ""}, {chardet, "single_byte.go", ""},
		{chardet, "multi_byte.go", ""}, {chardet, "2022.go", ""},
		{chardet, "utf8.go", ""}, {chardet, "unicode.go", ""}, {chardet, "recognizer.go", ""},
		{text, "encoding/simplifiedchinese/tables.go", ""},
	}
	ledger := []sourceFile{}
	retain := func(source, destination string, data []byte) {
		ledger = append(ledger, sourceFile{source, destination, fmt.Sprintf("%x", sha256.Sum256(data)), len(data)})
		if destination == "" {
			return
		}
		path := filepath.Join(root, filepath.FromSlash(destination))
		if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
			panic(err)
		}
		if err := os.WriteFile(path, data, 0644); err != nil {
			panic(err)
		}
	}
	for _, file := range files {
		data, err := os.ReadFile(filepath.Join(modules[file.module], filepath.FromSlash(file.source)))
		if err != nil {
			panic(err)
		}
		retain(file.module+"/"+file.source, file.destination, data)
	}
	for _, name := range []string{"LICENSE", "src/net/url/url.go"} {
		data, err := os.ReadFile(filepath.Join(runtime.GOROOT(), filepath.FromSlash(name)))
		if err != nil {
			panic(err)
		}
		retain(runtime.Version()+"/"+name, "", data)
	}
	const chromium = "https://raw.githubusercontent.com/chromium/dom-distiller/2a180397710719913340a12804affc65b789275e/LICENSE"
	client := &http.Client{Timeout: 30 * time.Second}
	response, err := client.Get(chromium)
	if err != nil {
		panic(err)
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		panic("Chromium license: " + response.Status)
	}
	data, err := io.ReadAll(io.LimitReader(response.Body, 100001))
	if err != nil {
		panic(err)
	}
	if len(data) > 100000 || !strings.Contains(string(data), "Copyright 2014 The Chromium Authors") || !strings.Contains(string(data), "END OF TERMS AND CONDITIONS") {
		panic("unexpected Chromium license contents")
	}
	retain(chromium, "licenses/LICENSE-chromium.txt", data)
	encoded, err := json.MarshalIndent(ledger, "", "  ")
	if err != nil {
		panic(err)
	}
	if err := os.MkdirAll(filepath.Join(root, "testdata"), 0755); err != nil {
		panic(err)
	}
	if err := os.WriteFile(filepath.Join(root, "testdata", "provenance.json"), append(encoded, '\n'), 0644); err != nil {
		panic(err)
	}
	fmt.Printf("Retained notices and %d source fingerprints\n", len(ledger))
}
