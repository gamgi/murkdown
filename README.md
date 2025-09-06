# Murkdown

An experimental semantic markup language and static site generator for composing and decomposing hypertext documents.

## Installation

Requires Rust **nightly** toolchain and Docker.

### Build from source

Replace `<ARCH>` below with your architecture, Eg. `aarch64` or `x86_64`.

```console
rustup target add <ARCH>-unknown-linux-musl
cargo build --target <ARCH>-unknown-linux-musl
ARCH=<ARCH> docker compose up --build
open http://localhost:8000/
```

## Quick start (playground)

Follow the instructions for [build from source](#build-from-source).
A live in-browser playground will be available at `http://localhost:8000/`.

## Quick start (command line)

Create the file `example.md` with the follow content:
```
# Exciting times!

You see, it's like Markdown on the surface.

You can write paragraphs.

* And
* Create
* Lists

> [!TIP]
> You can make callouts.

And that's where the similarities end.

> [!TABS]
>> [!CODE](language="python" id="foo")
>> def foo():
>>   print("hello world")
>
>> [!CODE](language="typescript" id="bar")
>> const bar = () => console.log("hello world")
>
>> [!CODE](language="plaintext" id="baz" src="archimedes")

And they can be composed, in exciting ways:

> [!LIST QUOTE](id="archimedes")
> The more you know, the more you know you don't know.
> Our problem is not that we aim too high and miss, but that we aim too low and hit.

That's enough to get you started.
```

Compile it by invoking the Murkdown cli:
```console
$ md build --as "simple website" ./example.md
# or
$ cargo run -- build --as "simple website" ./example.md
```

Open the result from `build/`:
```console
$ open build/example.html
```

It should look as follows:

![Example output](https://github.com/gamgi/murkdown/blob/main/example-output.png?raw=true)

## Examples

For more examples, head over to the [tests](https://github.com/gamgi/murkdown/tree/main/tests) and corresponding `*.in/` directories therein.

## Design

Some of the principles fueling the work:

* Literate programming
* Local first
* Worse is better
* Composability

Additional constraints motivated by curiosity and personal taste:

* Avoidance of start and end tags
* Avoidance of inline markup
* Avoidance of control structures and loops
* Avoidance of emoji

## License

The source code is licensed under the [AGPL v3 License](https://opensource.org/license/agpl-v3/).
