# language reference
unimap is a minimal programming language that allows you to do computations using pure pattern matching and data structure transformations, without the usage of any sort of control flow or math.

in unimap, you create functions that take data structures, match them against patterns, then map them to new data structures.

and with composition and piping, you walk up the abstraction layers until you create your own universe, a universe capable of computing your exact needs with just pure data transformation.

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

this document serve as a reference for the language, discribing its syntax and semantics, and detailing its execution model.

# general grammar
this document use the [gramex meta language](https://docs.rs/gramex/latest/gramex/docs/gram_ref/index.html) as a grammar notation, and expects the source to be a valid UTF-8 file.

unimap is whitespace insignificant where whitespace serve only to separate tokens, the whitespace characters are: space ` `, horizontal tab `\t`, line feed `\n` and carriage return `\r`.

all comma separated lists can have a trailing comma.

### comments
```gramex
let comment = "//" !"\n"* "\n"? | "/*" !"*/"* "*/";
```
unimap supports line and block comments in their respective c style syntax.

comments can be written in any position in the source, they are ignored by the parser.
```unimap
/* prints hello world */
fn main () => hello_world!; // => hello_world!
```

### identifiers
```gramex
let language_symbol = "." | ":" | "," | ";" | "=" | "|" | "/" | "(" | ")" | "{" | "}" | "[" | "]";
let language_keyword = "fn" | "let" | "symbol" | "import";
let ident = !({category("Z")} | {category("C")} | language_symbol)+ & !language_keyword & !nb &!"_";
```
an identifier is a sequence of characters that represent a name in the program.

an identifier can contain any unicode character except control, whitespace, unassingned or private characters (category `Z` and `C`) and symbols used by the language.

an identifier also can not be a underscore (`_`), a number or a language keyword.
```unimap
abc;
abc123;
_abc-123;
#$+-?;
λ😀ع↑;
```

# values and structures
```unimap
{ a: 1, 2: b, c: [1, 2, c] }
```
unimap features a unique and unconventional data model, its type system is not numeric based, it is purely symbolic and structured.

in unimap you model the world as symbols and data structures, these data structures are immutable and heterogeneous, and can be nested infinitely.

you dont mutate values, instead you create new structures, match them and against them, extract their inner values and transform them into new ones.

## symbol
symbols are globally unique unit primitives that represent a distinct value.

they are the fundamental primitive of the language and the units that build the other data structures.

they only match themself, and are semantically represented by an identifier.

```unimap
symbol a, b;

fn equal_a (v) => v: { a => 1, _ => 0 };

let test1 = equal_a(a); // => 1
let test2 = equal_a(b); // => 0
```
### symbol declaration
```gramex
let symbol_decl = "symbol" (ident ("{" list<ident, ","> "}")?)+ ";";
```
to avoid typos bugs, symbols can not be created out of thin air, they must be declared using the symbol declaration.

symbol declaration is a top level declaration that defines a list of local symbols.

symbol are not globally unique by name, multiple symbols in different modules can have the same name, but each retain its distinct value.

```unimap
// file_a
symbol a;
let other_a = a;

// file_b
import file_a { other_a };
symbol a;
let is_a = other_a: { a => 1, _ => 0 }; // => 0
```

### symbol enums
multiple symbols can be grouped together into a symbol enum.

symbol enums are defined inside a symbol declaration by following the symbol identifier with a comma separated list of variant identifiers enclosed inside a curly braces.

symbol variants acts like normal symbols, they have their own distinct value but are refered using `enum.symbol`.

enum symbols doesnt act like normal symbols, instead they serve as namespaces for symbols and act as a pattern that matches any of their variants.

```unimap
symbol Color { red, green, blue }, a;

fn is_red (v) => v: { Color.red => 1, _ => 0 };
fn is_color (v) => v: { Color => 1, _ => 0 };

let test1 = is_red(Color.red); // => 1
let test2 = is_color(Color.red); // => 1
let test2 = is_color(a); // => 0
```

## numbers
```gramex
let nb = "0" | "1".."9" ("0".."9")*;
```
numbers are special kind of symbols that have special properties.

numbers are arbitary sequences of decimal digits, starting with a non zero digit or being just `0`. 

they are defined intrinsicly by the language, globally unique, and are used to index arrays, other than these specific properties, numbers behave exactly like regular symbols.

```unimap
symbol 01;
fn is_1 (v) => v: { 1 => 1, _ => 0 };

let test1 = is_1(1); // => 1
let test2 = is_1(2); // => 0
let test2 = is_1(01); // => 0
```

## records
records are heterogeneous key value pair data structures that have symbols as fields, and any value as fields values (even other records).

they are defined by an optional list of `field = value` pairs separated by commas, enclosed inside curly braces.

they can be created, accessed via `rec.field` or `rec[index]`, matched or matched against, and passed to and from functions. like, any value in unimap, they are strictly immutable.

records equal each other if they have the exact fields with equal values.

```unimap
symbol a, b, c, deeply, nested, record;

let rec = { a = 1, b = 2, 3 = c };
let field_a = rec.a; // => 1
let field_b = rec[b]; // => 2

let rec2 = { deeply = { nested = { record = {} }, } };
let field_deep = rec2.deeply[nested]; // => { record = {} }

fn equal (a, b) => a: { b => 1, _ => 0 };
let test1 = equal(rec, rec); // => 1
let test3 = equal(rec, { a = 1, b = 2, 3 = c }); // => 1
let test2 = equal(rec, { a = 1, b = 2, 3 = a }); // => 0
let test3 = equal(rec, { a = 1, b = 2, 3 = c, 1 = a }); // => 0
```

## arrays
arrays are heterogeneous sequences of values, zero indexed by numbers.

they are defined by an optional comma separated list of values enclosed inside square brackets.

like records, they can be created, accessed by `arr.nb` or `arr[index]`, matched and are fully immutable.

arrays equal each other if they are equal in length and in all elements.

arrays are specialization / optimization of records with additional features (like spread pattern).

```unimap
symbol a, b, c, arr;

let arr = [1, 2, c];
let item_0 = arr[0]; // => 1
let item_2 = arr.2; // => c

let arr2 = [1, [a, b, c], { arr = [] }];

fn equal (a, b) => a: { b => 1, _ => 0 };
let test1 = equal(arr, arr); // => 1
let test2 = equal(arr, [1, 2, c]); // => 1
let test3 = equal(arr, [1, 2, a]); // => 0
let test4 = equal(arr, [1, 2, c, 1]); // => 0
```

# module system



# expressions

# patterns

# execution units