# unimap
build your universe throught matching and mapping

for more info see the [language reference](./docs/ref.md)

to install run
```sh
git clone --depth=1 https://github.com/aliibrahim123/unimap.git
cargo install --path ./unimap
```

# cli options
```
Usage: unimap [OPTIONS] <FILE>

Arguments:
  <FILE>  the entry file to process

Options:
  -b, --base-dir <DIR>             the imports root directory (default is the entry directory)
  -d, --debug-print <DEBUG_PRINT>  where to print `dbg` expressions [default: stdout] [possible values: silent, stdout, file]
      --debug-pretty               pretty print debug output
      --debug-file <DEBUG_FILE>    the debug output file if `debug_print` is `file`
  -o, --outfile <OUTFILE>          output file path (default to stdout)
      --no-pretty                  disable pretty printing of output
  -h, --help                       Print help
  -V, --version                    Print version
```

# examples
- [MEP](https://github.com/aliibrahim123/unimap-mep): mapped expressions processor, an advance 64 bit risc cpu.
- [unimap examples](https://github.com/aliibrahim123/unimap-examples): game of life and calculating pie.