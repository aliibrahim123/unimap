# language reference
unimap is a minimal programming language that allows you to do computations using pure pattern matching and data structure transformations, without the usage of any sort of control flow or math.

in unimap, you create functions that take data structures, match them against patterns, then map them to new data structures.

and with composition and piping, you walk up the abstraction layers until you create your own universe, a universe capable of computing your own needs with just pure data transformation.

here is a hello world example:
```unimap
symbol hello_world!;
fn main () => hello_world!;
```

here a better example:
```unimap
fn filter_zero (arr) => arr: {
	[] => [],
	[0, ..let tail] => filter_zero(tail),
	[let head, ..let tail] => [head, ..filter_zero(tail)]
};
fn map_one (arr) => arr: {
	[] => [],
	[1, ..let tail] => [2, ..map_one(tail)],
	[let head, ..let tail] => [head, ..map_one(tail)]
};
fn main () => filter_zero([1, 0, 0, 2, 0, 3]) |> map_one(_); // => [2, 2, 3]
```

this document serve as a reference for the language, discribing its syntax and semantics and detailing its execution model.

# general grammer
this document use the [gramex meta language](https://docs.rs/gramex/latest/gramex/docs/gram_ref/index.html) as a grammer notation, and expects the source to be a valid UTF-8 file.

unimap is whitespace insignificant where whitespace serve only to separate tokens, the whitespace characters are: space ` `, horizontal tab `\t`, line feed `\n` and carriage return `\r`.

### comments
unimap supports line and block comments in their respective c style syntax.

comments can be written in any position in the source, they are ignored by the parser.

```gramex
let comment = "//" !"\n"* "\n"? | "/*" !"*/"* "*/";
```
#### example
```unimap
/* prints hello world */
fn main () => hello_world!; // => hello_world!
```

### identifiers
an identifier is a sequence of characters that represent a name in the program.

an identifier can contain any unicode character except control, whitespace, unassingned or private characters (category `Z` and `C`) and symbols used by the language.

an identifier also can not be a dash (`_`), a number or a language keyword.

```
let language_symbol = "." | ":" | "," | ";" | "=" | "|" | "/" | "(" | ")" | "{" | "}" | "[" | "]";
let language_keyword = "fn" | "let" | "symbol" | "import";
let ident = !({category("Z")} | {category("C")} | language_symbol)+ & !language_keyword & !nb &!"_";
```
#### example
```
abc;
abc123;
_abc-123;
#$+-?;
λ😀ع↑;
```

# values and structures
## symbol
