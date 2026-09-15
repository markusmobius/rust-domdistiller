package main

import (
	"bytes"
	"fmt"
	"go/ast"
	"go/parser"
	"go/token"
	"io"
	"math/rand"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"sort"
	"strconv"
	"strings"

	"github.com/gogs/chardet"
	"golang.org/x/net/html/charset"
	xunicode "golang.org/x/text/encoding/unicode"
	"golang.org/x/text/runes"
	"golang.org/x/text/transform"
	"golang.org/x/text/unicode/norm"
)

type decodingCase struct {
	Charset string `json:"charset"`
	Input   []int  `json:"input"`
	Output  string `json:"output"`
}

func decodeReference(input []byte, label string) []byte {
	encoding, _ := charset.Lookup(label)
	if encoding == nil {
		encoding = xunicode.UTF8
	}
	decoded := transform.NewReader(bytes.NewReader(input), encoding.NewDecoder())
	normalizer := transform.Chain(norm.NFD, runes.Remove(runes.Predicate(func(character rune) bool { return character == '\u00ad' })), norm.NFC)
	output, err := io.ReadAll(transform.NewReader(decoded, normalizer))
	if err != nil {
		panic(err)
	}
	return output
}

func decodingCases() []decodingCase {
	cases := []decodingCase{}
	inputs := [][]byte{{}, []byte("Cafe\u0301 co\u00adoperate \u1100\u1161"), {0xef, 0xbb, 0xbf, 'a'}, {0xff, 0xfe, 0x61, 0}, {0xfe, 0xff, 0, 0x61}}
	for value := 0; value < 256; value++ {
		inputs = append(inputs, []byte{byte(value)})
	}
	for _, first := range []byte{0x80, 0x81, 0x8e, 0x8f, 0xa0, 0xa1, 0xc0, 0xc2, 0xe0, 0xed, 0xf0, 0xf4, 0xfe, 0xff} {
		for _, second := range []byte{0, 0x20, 0x3c, 0x40, 0x7f, 0x80, 0x9f, 0xa0, 0xa1, 0xbf, 0xfe, 0xff} {
			inputs = append(inputs, []byte{first, second}, []byte{first, second, 0x81}, []byte{first, second, 0x81, 0x30})
		}
	}
	for _, label := range []string{"UTF-8", "UTF-16BE", "UTF-16LE", "UTF-32BE", "UTF-32LE", "ISO-8859-1", "windows-1252", "ISO-8859-2", "windows-1250", "ISO-8859-5", "ISO-8859-6", "ISO-8859-7", "windows-1253", "ISO-8859-8-I", "ISO-8859-8", "windows-1255", "windows-1251", "windows-1256", "KOI8-R", "ISO-8859-9", "windows-1254", "Shift_JIS", "GB18030", "EUC-JP", "EUC-KR", "Big5", "ISO-2022-JP", "ISO-2022-KR", "ISO-2022-CN", "IBM424_rtl", "IBM424_ltr", "IBM420_rtl", "IBM420_ltr"} {
		for _, input := range inputs {
			output := decodeReference(input, label)
			entry := decodingCase{Charset: label, Input: []int{}, Output: string(output)}
			for _, value := range input {
				entry.Input = append(entry.Input, int(value))
			}
			cases = append(cases, entry)
		}
	}
	return cases
}

type encodingCase struct {
	Input  []int          `json:"input"`
	Scores map[string]int `json:"scores"`
}

func encodingCases(readers []readerCase) []encodingCase {
	inputs := [][]byte{}
	for _, reader := range readers {
		input := make([]byte, len(reader.Input))
		for index, value := range reader.Input {
			input[index] = byte(value)
		}
		inputs = append(inputs, input)
	}
	for _, length := range []int{0, 1, 2, 3, 4, 5, 9, 10, 11, 39, 40, 41, 599, 600, 601, 8191, 8192, 8193, 12000} {
		for _, pattern := range [][]byte{
			{0}, {0xff}, {0xef, 0xbb, 0xbf}, {0xc0, 0x80}, {0xed, 0xa0, 0x80}, {0xf0, 0x90, 0x80, 0x80},
			{0x8f, 0xa1, 0xa2}, {0x81, 0x30, 0x81, 0x30}, {0x1b, 0x24, 0x42, 0x0e, 0x0f},
			[]byte("the quick article and the latest science news "), []byte("<b><i><span><p>the latest news</p></span></i></b>"),
			[]byte("<<p<<<span>the latest news</span></p>"),
		} {
			input := make([]byte, length)
			for index := range input {
				input[index] = pattern[index%len(pattern)]
			}
			inputs = append(inputs, input)
		}
	}
	random := rand.New(rand.NewSource(20260917))
	for index := 0; index < 300; index++ {
		input := make([]byte, random.Intn(1024))
		if _, err := random.Read(input); err != nil {
			panic(err)
		}
		inputs = append(inputs, input)
	}
	cases := []encodingCase{}
	for _, input := range inputs {
		entry := encodingCase{Input: []int{}, Scores: map[string]int{}}
		for _, value := range input {
			entry.Input = append(entry.Input, int(value))
		}
		results, err := chardet.NewHtmlDetector().DetectAll(input)
		if err != nil && err != chardet.NotDetectedError {
			panic(err)
		}
		for _, result := range results {
			entry.Scores[result.Charset] = result.Confidence
		}
		cases = append(cases, entry)
	}
	return cases
}

func exportEncodingTables(output string) {
	directory, err := exec.Command(filepath.Join(runtime.GOROOT(), "bin", "go"), "list", "-mod=readonly", "-m", "-f", "{{.Dir}}", "github.com/gogs/chardet").Output()
	if err != nil {
		panic(err)
	}
	root := strings.TrimSpace(string(directory))
	values := map[string]ast.Expr{}
	functions := map[string]*ast.FuncDecl{}
	for _, name := range []string{"detector.go", "single_byte.go", "multi_byte.go", "2022.go"} {
		file, err := parser.ParseFile(token.NewFileSet(), filepath.Join(root, name), nil, 0)
		if err != nil {
			panic(err)
		}
		for _, declaration := range file.Decls {
			switch declaration := declaration.(type) {
			case *ast.GenDecl:
				for _, spec := range declaration.Specs {
					if spec, ok := spec.(*ast.ValueSpec); ok {
						for index, name := range spec.Names {
							if index < len(spec.Values) {
								values[name.Name] = spec.Values[index]
							}
						}
					}
				}
			case *ast.FuncDecl:
				functions[declaration.Name.Name] = declaration
			}
		}
	}
	var generated bytes.Buffer
	names := []string{}
	for name := range values {
		if strings.HasPrefix(name, "charMap_") || strings.HasPrefix(name, "ngrams_") || strings.HasPrefix(name, "commonChars_") || strings.HasPrefix(name, "escapeSequences_") {
			names = append(names, name)
		}
	}
	sort.Strings(names)
	for _, name := range names {
		array := values[name].(*ast.CompositeLit)
		typeName := "u32"
		if strings.HasPrefix(name, "charMap_") {
			typeName = "u8"
		}
		if strings.HasPrefix(name, "commonChars_") {
			typeName = "u16"
		}
		if strings.HasPrefix(name, "escapeSequences_") {
			typeName = "&[u8]"
		}
		fmt.Fprintf(&generated, "pub(super) const %s: &[%s] = &[\n", strings.ToUpper(name), typeName)
		for _, element := range array.Elts {
			if inner, ok := element.(*ast.CompositeLit); ok {
				generated.WriteString("    &[")
				for _, item := range inner.Elts {
					fmt.Fprintf(&generated, "%s, ", item.(*ast.BasicLit).Value)
				}
				generated.WriteString("],\n")
			} else {
				fmt.Fprintf(&generated, "    %s,\n", element.(*ast.BasicLit).Value)
			}
		}
		generated.WriteString("];\n\n")
	}
	var resolve func(ast.Expr, map[string]ast.Expr) ast.Expr
	resolve = func(expression ast.Expr, bindings map[string]ast.Expr) ast.Expr {
		switch expression := expression.(type) {
		case *ast.Ident:
			if value, ok := bindings[expression.Name]; ok {
				return value
			}
			return expression
		case *ast.UnaryExpr:
			return resolve(expression.X, bindings)
		default:
			return expression
		}
	}
	var constructor func(*ast.CallExpr) *ast.CompositeLit
	constructor = func(call *ast.CallExpr) *ast.CompositeLit {
		function := functions[call.Fun.(*ast.Ident).Name]
		bindings := map[string]ast.Expr{}
		position := 0
		for _, parameter := range function.Type.Params.List {
			for _, name := range parameter.Names {
				bindings[name.Name] = resolve(call.Args[position], nil)
				position++
			}
		}
		for _, statement := range function.Body.List {
			if assignment, ok := statement.(*ast.AssignStmt); ok {
				for index, target := range assignment.Lhs {
					value := resolve(assignment.Rhs[index], bindings)
					if nested, ok := value.(*ast.CallExpr); ok {
						copyCall := *nested
						copyCall.Args = make([]ast.Expr, len(nested.Args))
						for index, argument := range nested.Args {
							copyCall.Args[index] = resolve(argument, bindings)
						}
						value = constructor(&copyCall)
					}
					switch target := target.(type) {
					case *ast.Ident:
						bindings[target.Name] = value
					case *ast.SelectorExpr:
						model := resolve(target.X, bindings).(*ast.CompositeLit)
						for _, field := range model.Elts {
							field := field.(*ast.KeyValueExpr)
							if field.Key.(*ast.Ident).Name == target.Sel.Name {
								field.Value = value
							}
						}
					default:
						panic("unsupported constructor assignment")
					}
				}
				continue
			}
			returned, ok := statement.(*ast.ReturnStmt)
			if !ok {
				continue
			}
			expression := resolve(returned.Results[0], bindings)
			if nested, ok := expression.(*ast.CallExpr); ok {
				for index := range nested.Args {
					nested.Args[index] = resolve(nested.Args[index], bindings)
				}
				return constructor(nested)
			}
			result := expression.(*ast.CompositeLit)
			copy := *result
			copy.Elts = make([]ast.Expr, len(result.Elts))
			for index, field := range result.Elts {
				if keyed, ok := field.(*ast.KeyValueExpr); ok {
					copyField := *keyed
					copyField.Value = resolve(keyed.Value, bindings)
					copy.Elts[index] = &copyField
				} else {
					copy.Elts[index] = resolve(field, bindings)
				}
			}
			return &copy
		}
		panic("constructor has no return")
	}
	text := func(expression ast.Expr) string {
		switch expression := expression.(type) {
		case *ast.BasicLit:
			value, err := strconv.Unquote(expression.Value)
			if err != nil {
				panic(err)
			}
			return strconv.Quote(value)
		case *ast.Ident:
			return strings.ToUpper(expression.Name)
		default:
			panic(fmt.Sprintf("unsupported table value %T", expression))
		}
	}
	var single, multiple, iso bytes.Buffer
	counts := [3]int{}
	for _, expression := range values["recognizers"].(*ast.CompositeLit).Elts {
		call := expression.(*ast.CallExpr)
		name := call.Fun.(*ast.Ident).Name
		if strings.HasPrefix(name, "newRecognizer_utf") {
			continue
		}
		model := constructor(call)
		switch model.Type.(*ast.Ident).Name {
		case "recognizerSingleByte":
			fields := map[string]string{}
			for _, field := range model.Elts {
				field := field.(*ast.KeyValueExpr)
				fields[field.Key.(*ast.Ident).Name] = text(field.Value)
			}
			if fields["hasC1ByteCharset"] == "" {
				fields["hasC1ByteCharset"] = `""`
			}
			fmt.Fprintf(&single, "    (%s, %s, %s, %s),\n", fields["charset"], fields["hasC1ByteCharset"], fields["charMap"], fields["ngram"])
			counts[0]++
		case "recognizerMultiByte":
			decoder := model.Elts[2].(*ast.CompositeLit).Type.(*ast.Ident).Name
			fmt.Fprintf(&multiple, "    (%s, %q, %s),\n", text(model.Elts[0]), strings.TrimPrefix(decoder, "charDecoder_"), text(model.Elts[3]))
			counts[1]++
		case "recognizer2022":
			fmt.Fprintf(&iso, "    (%s, %s),\n", text(model.Elts[0]), text(model.Elts[1]))
			counts[2]++
		default:
			panic("unsupported recognizer")
		}
	}
	fmt.Fprintf(&generated, "pub(super) const SINGLE_BYTE: &[(&str, &str, &[u8], &[u32])] = &[\n%s];\n\n", single.String())
	fmt.Fprintf(&generated, "pub(super) const MULTI_BYTE: &[(&str, &str, &[u16])] = &[\n%s];\n\n", multiple.String())
	fmt.Fprintf(&generated, "pub(super) const ISO_2022: &[(&str, &[&[u8]])] = &[\n%s];\n", iso.String())
	if counts != [3]int{27, 5, 3} {
		panic(fmt.Sprintf("unexpected model counts: %v", counts))
	}
	directory, err = exec.Command(filepath.Join(runtime.GOROOT(), "bin", "go"), "list", "-mod=readonly", "-m", "-f", "{{.Dir}}", "golang.org/x/text").Output()
	if err != nil {
		panic(err)
	}
	file, err := parser.ParseFile(token.NewFileSet(), filepath.Join(strings.TrimSpace(string(directory)), "encoding", "simplifiedchinese", "tables.go"), nil, 0)
	if err != nil {
		panic(err)
	}
	for _, declaration := range file.Decls {
		general, ok := declaration.(*ast.GenDecl)
		if !ok {
			continue
		}
		for _, spec := range general.Specs {
			value, ok := spec.(*ast.ValueSpec)
			if !ok || len(value.Names) != 1 {
				continue
			}
			name := value.Names[0].Name
			if name != "decode" && name != "gb18030" {
				continue
			}
			array := value.Values[0].(*ast.CompositeLit)
			if name == "gb18030" {
				generated.WriteString("\npub(super) const GB_RANGES: &[(u32, u32)] = &[\n")
				for _, element := range array.Elts {
					pair := element.(*ast.CompositeLit)
					fmt.Fprintf(&generated, "    (%s, %s),\n", pair.Elts[0].(*ast.BasicLit).Value, pair.Elts[1].(*ast.BasicLit).Value)
				}
			} else {
				generated.WriteString("\npub(super) const GB_DECODE: &[u16] = &[\n")
				position := 0
				for _, element := range array.Elts {
					keyed := element.(*ast.KeyValueExpr)
					index, err := strconv.Atoi(keyed.Key.(*ast.BasicLit).Value)
					if err != nil {
						panic(err)
					}
					for position < index {
						generated.WriteString("    0,\n")
						position++
					}
					fmt.Fprintf(&generated, "    %s,\n", keyed.Value.(*ast.BasicLit).Value)
					position++
				}
			}
			generated.WriteString("];\n")
		}
	}
	if err := os.MkdirAll(filepath.Dir(output), 0755); err != nil {
		panic(err)
	}
	if err := os.WriteFile(output, generated.Bytes(), 0644); err != nil {
		panic(err)
	}
	fmt.Printf("Exported %d recognition tables and %v models from pinned chardet\n", len(names), counts)
}
