Verbatim code can be placed in blocks, like this:

> [!CODE](id="fibonacci.py" language="python")
> # fib.py
>
> def fib(n):
>   if n < 2:
>     return n
>   else:
>     return fib(n-1) + fib(n-2)

The code can then be referenced in future blocks.

> [!CODE](src="fibonacci.py" language="shell")
> #!/bin/python
> ...
>
> if __name__ == "__main__":
>   result = fib(5)
>   print("fibonacci result is", result)

And nested in blocks.

> Like this:
>
>> [!CODE](src="fibonacci.py" language="python")
