<p>
  Verbatim code can be placed in blocks, like this:
</p>

<pre><code data-noescape data-language-python data-line-numbers># fib.py

def fib(n):
  if n &lt; 2:
    return n
  else:
    return fib(n-1) + fib(n-2)
</code></pre>

<p>
  The code can then be referenced in future blocks.
</p>

<pre><code data-noescape data-language-shell data-line-numbers>#!/bin/python
# fib.py

def fib(n):
  if n &lt; 2:
    return n
  else:
    return fib(n-1) + fib(n-2)

if __name__ == "__main__":
  result = fib(5)
  print("fibonacci result is", result)
</code></pre>

<p>
  And nested in blocks.
</p>

<div>
  <p>
    Like this:
  </p>
  
  <pre><code data-noescape data-language-python data-line-numbers># fib.py

def fib(n):
  if n &lt; 2:
    return n
  else:
    return fib(n-1) + fib(n-2)
</code></pre>
</div>
