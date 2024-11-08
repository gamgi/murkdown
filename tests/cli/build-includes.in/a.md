This file includes another

> [!FOO](src="b.md" id="foo")
> This is overwritten

then it includes the source of itself

> [!BAR](src="file:a.md")
> Ad Infinitum

> [!BAZ](src="#foo")
> This is also overwritten

and that is that.
