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

this languages is not designed for production, it is not grandate to be updated or fixed, nor being backward compatible, use it for fun, not for production.

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
### `symbol` declaration
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

they are defined by a list of `field = value` pairs separated by commas, enclosed inside curly braces.

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

they are defined by a comma separated list of values enclosed inside square brackets.

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
unimap has a very simple module system where each file is a distinct module having its own scope and path.

each module consists of a list of top level declarations that defines a number of local items.

these declarations are `import`, `fn`, constant and `symbol` declerations.

these declarations can be ordered in any way, and can reference other items declared / imported after them.

the declared items are local to the module, however they can be imported into other modules with the `import` statement.

each item must have a locally unique identifier, moreover, multiple items in different modules can be decleared with the same name totally fine until you want to import them.

```unimap
symbol a, b, c;
let arr = [a, b, f(), d];
import other { d };
fn f () => c;
```

## `import` declaration
```gramex
let import_decl = "import" path "{" list<ident, ","> "}" ";";
let path = list<ident, ".">;
```
`import` declerations bring number of items declared in another module into the current module scope.

`import` declerations consists of a path to the source module and a comma separated list of item identifiers to import.

the imported items must not conflict with locally declared items, or other imported items, and like local items can be used before the `import` declaration.

```
// file a.unim
symbol a, b, c;

// file b.unim
let arr = [a, b, c];
import a { a, b, c };
```

the path is a dot separated list of identifiers that refers to a specific module, they are relative to the base directory, and are converted to fs path in form of `some.file` -> `{base_dir}/some/file.unim`.

a path part can not resolve to directory and a file in the same time
```unimap
// is base resolve to base.unim or to base/
import base { a, b }; 
import base.sub { c, d }; 
```

modules can import from each other in any way, a cyclic graph is possible not necessary to be in a tree like order.
```unim
// file a
symbol a, b, c;
import b { d };
fn f () => d;

// file b
let d = [a, b, c];
import a { a, b, c };
```

# expressions
unimap is an expression based language, it is build around expressions, operations and literals that evaluate to values.

expressions are devided according to associativity and heirarchy into primary, postfix and flow.

## primary expressions
```gramex
let primary_expr = "_" | ident | nb | call_expr | "(" expr ")" | arr_expr | rec_expr | dbg_expr;
```
primary expressions are the top level expressions, they create / resolve to values which the rest of the expressions operate on.

### intermediate expression
the intermediate expression (`_`) evaluate to the value piped from the previous expression in the pipe chain.

it is forbidden to use `_` outside a pipe chain.

```unimap
symbol a, b, c, nested;
let result = 1 |> [_, 2, 3] |> { a = _[0], b = _[1], c = { nested = _[2] } }; // => { a = 1, b = 2, c = { nested = 3 } }
```

### grouped expression
the grouped expression is an expression enclosed inside parentheses, it evaluates to the value of the expression inside.

```unimap
let one = (1 |> [_, 2, 3]).0; // => 1
```

### identifier expression
the identifier expression is an identifier that evaluates to the value of the symbol / constant / local resolved by the identifier in the current scope.

```unimap
symbol a;
let b = 1;
fn f (c) => [a, b, c];
```

### call expression
```gramex
let call_expr = ident "(" list<expr, ",">? ")";
```
the call expression is an expression that calls a function by its identifier with a list of arguments, evaluating to its result.

the argument list is a comma separated list of expressions enclosed inside parentheses.

```unimap
fn f1 (a, b, c) => [a, b, c];
fn f2 () => 3;
let result = f1(1, 2, f2()); // => [1, 2, 3]
```

### debug expression
```gramex
let dbg_expr = "dbg" "(" expr ","? ")";
```
the debug expression is like a call expression to a `dbg` function, it prints the value of the passed argument then returns it.

if a local item is named `dbg`, a call to `dbg` will become a call to the local item not a debug expression.

```unimap
let value = [1, 2, 3] |> dbg(_) |> some_fn(_);
```

## literals
literals are primary expressions that creates values.

### number literal
the number literal is a number literal that evaluates to a number symbol.

```unimap
let nb = 1;
```
### array literal
```gramex
let arr_expr = "[" list<expr | ".." expr, ",">? "]";
```
the array literal is an expression that creates an array value from a list of elements.

the item list is a comma separated list enclosed inside brackets, consisting of:
- expressions with their values appended to the array.
- spread operator `..` followed by an expression that evaluates to an array whose items are appended to the array.

```unimap
let arr1 = [1, 2, 3];
let arr2 = [0, ..arr1, 4, ..[5, 6]]; // => [0, 1, 2, 3, 4, 5, 6]
```

### record literal
```gramex
let field = ident | symbol;
let rec_expr = "{" list<rec_item, ",">? "}";
let rec_item = field "=" expr | "[" expr "]" "=" expr | ".." expr;
```
the record literal is an expression that creates a record value from a list of record items.

the record item list is a comma separated list enclosed inside curly braces, consisting of:
- a field item: a field symbol and its value expression separated by `=`.
- indexed item: an expression enclosed inside square brackets evaluating to the field symbol, then a `=` and the field value expression.
- spread operator `..` followed by an expression that evaluates to a record whose fields are added to the record.

if two items evaluate to the same field symbol, the last one wins.

```unimap
symbol a, b, c, e { v1, v2 };
let rec1 = { a = 1, b = 2, 3 = c };
let rec2 = { a = 2, ..rec1, [e.v1] = e.v2, } // => { a = 1, b = 2, 3 = c, e.v1 = e.v2 };
```

## postfix expressions
```gramex
let postfix_expr = primary_expr postfix_op*;
let postfix_op = field_expr | index_expr | map_expr;
```
postfix expressions are primary expressions followed by a chain of postfix operators that operate on the previous expression value.

### field access operator
```gramex
let field_expr = "." field;
```
the field access operator evaluates to the value of a field of a record or an item of an array.

it is written as a `.` followed by a field symbol, if the record / array doesnt have the field / item, an error is raised.

the field access operator is also used in resolving symbol variants.

```unimap
symbol a, b, c, e { v1, v2 };
let rec = { a = 1, b = 2, 3 = c };
let arr = [a, b, 3];

let field_a = rec.a; // => 1
let item_2 = arr.1; // => 2
let variant = e.v1;
```

### index operator
```gramex
let index_expr = "[" expr "]";
```
the index operator index into an array or a record and evaluates to the indexed item / field value.

it is written as an expression enclosed inside square brackets, if the array / record doesnt have the index value, an error is raised.

```unimap
symbol a, b, c, e { v1, v2 };
let rec = { a = 1, b = 2, 3 = c };
let arr = [a, b, 3];
let rec2 = { e.v1 = [e.v2] };

let field_a = rec[a]; // => 1
let item_2 = arr[1]; // => 2
let nested = rec2[e.v1].0; // => e.v2
```

## map operator
```gramex
let map_expr = ":" "{" list<pat "=>" expr, ","> "}";
```
the map operator is the heart of the language.

it is a postfix operator that takes the previous expression value, match it against a set of patterns, then map the value using the first match map expression.

if the patterns are not exhaustive, an error is raised.

it is a `:` followed by a comma separated list of match arms enclosed inside curly braces, each arm has a pattern and a map expression separated by `=>`.

the map expression is the only control flow inside the language.

```unimap
symbol a, b, c;
fn match (v) => v: {
	a => 1,
	b => 2,
	_ => 3,
};

let result1 = match(a); // => 1
let result2 = match(c); // => 3
```

## flow expression
```
let expr = postfix_expr | pipe_expr;
```
flow expressions are the entry level of the expression heierarchy, they may be a regular primary expression, postfix expression or a pipe expression.

### pipe expression
```gramex
let pipe_expr = list<expr, "|>">;
```
the pipe expression is a chain of expressions separated by the pipe operator `|>` that evaluates in sequence.

each expression value is pipe to the next expression in the chain till the end expression where its value is the evaluated value of the whole chain.

the pipe expression with the map expression are the only flow inside the language.

```unimap
let result = 1   // => 3
	|> [_, 2, 3]
	|> dbg(_)
	|> _[2]
```

# patterns
```gramex
let pat = `_` | ident | nb | ident "." ident | let_pat | rec_pat | arr_pat | or_pat;
```
pattern matching is the identity of the language.

patterns are used to match the equality and the structure of values, they are also used in destructing the values.

### wildcard pattern
the wildcard pattern `_` matches any value.

```unimap
let result = 0: { _ => 1 } // => 1
```

### identifier pattern
the identifier pattern is an identifier that matches by equality to the value of the symbol / constant / local resolved by the identifier in the current scope.

```unimap
symbol a, e { v1, v2 };
let b = 1;
fn match (v, c) => v: {
	a => 1,
	b => 2,
	c => 3,
	e => 4,
};

let test1 = match(a, 2); // => 1
let test2 = match(1, 2); // => 2
let test3 = match(2, 2); // => 3
let test4 = match(e.v1, 2); // => 4
```

### number pattern
the number pattern is a number that matches itself.

```unimap
fn is_1 (v) => v: { 1 => 1, _ => 0 };
let test1 = is_1(1); // => 1
let test2 = is_1(0); // => 0
```

### variant pattern
the variant pattern is a pattern that matches by a symbol variant.

it is written as the enum identifier followed by the variant identifier separated by a `.`.

```unimap
symbol e { v1, v2 };
fn is_v1 (v) => v: { e.v1 => 1, _ => 0 };
let test1 = is_v1(e.v1); // => 1
let test2 = is_v1(e.v2); // => 0
```

## let pattern
```gramex
let let_pat = "let" ident (":" pat)?;
```
the let pattern matches the value by a pattern and assign it to a local if it matches.

the let pattern is written as `let` followed by the local identifier and an optional pattern preceeded by a `:`.

if no pattern is given, the let pattern always matches.

```unimap
fn map (v, from, to) => v: {
	from => to,
	let x => x,
};

let test1 = map(1, 1, 0); // => 0
let test2 = map(2, 1, 0); // => 2
```

## array pattern
```gramex
let arr_pat = "[" list<pat, ",">? (","? ".." pat?)? "]";
```
the array pattern matches an array value by destructuring its items and matching them against a pattern list.

this list is a comma separated one enclosed inside brackets.

### rest pattern
the array pattern optionally takes a rest pattern that slices the rest of the items into an array value and matches a pattern against it.

the rest pattern must be the last item, it starts with a `..` followed by an optional pattern, if no pattern is given, the rest pattern always matches.

the rest pattern will slice to an empty slice if there is no items left instead of failing.

if no rest pattern is given, the matched array must have the same length as the pattern list.

```unimap
fn filter (v, filtered) => v: {
	[] => [],
	[filtered, ..let rest] => filter(rest, filtered),
	[let item, ..let rest] => [item, ..filter(rest, filtered)],
};

let result = filter([1, 2, 3], 2); // => [1, 3]
```

## record pattern
```gramex
let rec_pat = "{" list<field_pat, ","> "}";
let field_pat = field ":" pat | "[" expr "]" ":" pat | let ident (":" pat)?;
```
the record pattern matches a record value on the individual field level.

it takes a comma separated list of field patterns enclosed inside curly braces, these patterns can be
- direct: a field symbol followed by a `:` then the value pattern.
- indexed: an expression enclosed inside brackets evaluating to a field symbol, followed by a `:` then the value pattern.
- let shorthand: a shorthand for `field: let field: pat` where the field symbol and the local share the same identifier.     
the value pattern is optional like the let pattern.

the record must have the exact fields as the field patterns, but it can have extra unmatched fields.

```unimap
symbol a, b, c;
fn match (v, f) => v: {
	{ a: 1, [f]: b } => 1,
	{ a: 1, let b, c: let d } => b[d],
	_ => 0,
};

let test1 = match({ a = 1, b = b }, b); // => 1
let test2 = match({ a = 1, b = [1, 2, 3], c = 2 }, b); // => 3;
let test3 = match({ a = 1 }, b); // => 0
let test4 = match({ a = 1, b = b, c = 2 }, b); // => 1
```

## or pattern
```gramex
let or_pat = list<pat, "|">;
```
the or pattern matches by any of the patterns in a `|` separated list.

or pattern can not have any let pattern inside it.

```unimap
fn match (v) => v: {
	1 | 2 | 3 => 1,
	_ => 0,
};

let test1 = match(2); // => 1
let test2 = match(4); // => 0
```

# execution units
## functions
```gramex
let fn_decl = "fn" ident "(" lisr<ident, ","> ")" "=>" expr ";";
```
function declaration is a top level declaration that defines a function.

it consists of a function identifier, a comma separated list of argument identifiers enclosed inside parentheses, `=>` and a body expression.

functions are items called by the call expression, they take a number of arguments, evaluate the body expression with them, then return the result.

they can be called recursively until the stack overflows.

arguments are used inside the body expression throught the identifier expression and pattern.

functions are composed of one body expression, if you want sequential execution, use the pipe operator `|>` instead.

```unimap
fn filter (v, filtered) => v: {
	[] => [],
	[filtered, ..let rest] => filter(rest, filtered),
	[let item, ..let rest] => [item, ..filter(rest, filtered)],
};
let result = filter([1, 2, 3], 2); // => [1, 3]
```

## constants
```gramex
let const_decl = "let" ident "=" expr ";";
```
constant declaration is a top level declaration that defines a constant.

it consists of a constant identifier followed by an `=` and an init expression.

constants are items that stores a constant value, these values are evaluated from the init expression.

they can be retrived inside the current module expressions through the identifier expression and pattern.

constants are lazy evaluated, they may not evaluate, but they are evaluated only once during the program execution.

```unimap
let a = 1;
let b = [1, 2, 3];
fn f () => b[a];
```

## execution
the execution in unimap is compute and print model.

the runtime look in the entry module to determine the execution mode it will use.

### main mode
the default execution mode, the execution start in a function called `main` in the entry module.

it is a function taking no arguments, it is called by the runtime one time and its result is printed as an output.

```unimap
symbol hello_world!;
fn main () => hello_world!;
```

### continous mode
the continous mode is a secondary mode if no `main` function is provided, inside it a `loop` function is called continously accumulating a value.

the runtime first call an `init` function in the entry module, a zero argument function that provide the initial value of the accumulator.

then it calls the `loop` function continously, each time the `loop` takes the accumulator as an argument and a 2 item array acting as`[control, acc]` tuple.

- `control`: a symbol that controls the loop, it can have 2 names: `continue` to advance the loop, and `end` to stop it.
- `acc`: the new accumulator for the next iteration, or the final output if `control` is `end`.

the continous mode is an optimization for programs that run in a loop model with a huge iteration count, as the stack limit will be reached because of the very deep recursion. 

```unimap
symbol continue, end;
// print numbers 0 to 9
fn init () => [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
fn loop (acc) => acc: {
	[let last] => [end, last],
	[let cur, ..let rest] => dbg(cur) |> [continue, rest],
};
```