
A Let's do some Fibonacci numbers.
Define the function:

> [!CODE](language="python" id="fib")
> def fib(n):
>   if n < 2:
>     return n
>   else:
>     return fib(n-1) + fib(n-2)

And call it like this:

> [!CODE](language="python" src="fib" id="fibmain")
> #!/usr/bin/env python
>
> ...
>
> for i in range(4):
>   print(fib(i))

The result will be:

* [!](src="exec:fibmain")
* This will be a list of fibonacci numbers

We can also call external scripts.

> [!](src="exec:code.sh")
> This will be a hello world
