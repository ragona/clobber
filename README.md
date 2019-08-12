# clobber

`clobber` is a simple TCP load testing tool, written in Rust. It uses the `async/await` syntax, which currently
requires the nightly branch, but is targeted to stabilize in the `1.38` release. This project was created as a way to
kick the tires of the new syntax, since a network I/O heavy tool is a great use case for an async concurrency model.

## Usage
```
clobber 0.1
tcp load testing tool

USAGE:
    clobber [FLAGS] [OPTIONS] --target <target>

FLAGS:
    -h, --help       Prints help information
    -v               Sets the log level, from -v to -vvv
    -V, --version    Prints version information

OPTIONS:
        --connect-timeout <connect-timeout>    Timeout for initial TCP syn timeout
    -c, --connections <connections>            Max number of open connections at any given time
    -d, --duration <duration>                  Length of the run (e.g. 5s, 10m, 2h, etc...)
        --rate <rate>                          Limit to a particular rate per-second.
        --read-timeout <read-timeout>          Timeout for reading response from target
    -t, --target <target>                      Host to clobber
        --threads <threads>                    Number of threads

```

## Goals

### 1. High throughput

Generating enough load to test large distributable services can require a
cost-prohibitive number of hosts to send requests. I wanted to try to make a tool that
prioritized performance. This requires some tradeoffs, such as not precisely controlling the rate
of requests. We can get this mostly correct, but without communication between threads
that's the best we can do.

<details>
<summary>Strategies to improve throughput</summary>

#### - Thread local

This library uses no cross-thread communication via `std::sync` or `crossbeam`.
All futures are executed on a `LocalPool`, and the number of OS threads used is configurable.
Work-stealing has an overhead that isn't suitable for this kind of use case. This has a
number of design impacts. For example, it becomes more difficult to aggregate what each
connection is doing. This is simple if you just pass the results to a channel, but this
has a non-trivial impact on performance.

Note: This is currently violated by the way this library accomplishes rate limiting, which
relies on a global thread that manages timers. This ends up putting disproportionate load
on that thread at some point which impacts performance.

#### - Limit open ports and files

Two of the key limiting factors for high TCP client throughput are running out of ports,
or opening more files than the underlying OS will allow. `clobber` tries to minimize issues
here by giving users control over the max connections. It's also a good idea to check out
your specific `ulimit -n` settings and raise the max number of open files.
</details>

### 2. Async/Await

A high-throughput network client is a classic example of an application that
is suitable for an async concurrency model. This is possible with tools like `tokio` and
`hyper`, but they currently use a futures model that requires a somewhat non-ergonomic
coding style with a tricky learning curve.

At the time of this writing, Rust's async/await syntax is not quite stable, but it
is available on the nightly branch. The new syntax is a huge improvement in readability
over the current Futures-based concurrency model. When async/await is moved to the stable
branch in version 1.38 this library will move to the stable branch as well.

### 3. Simple and Readable Code

One of the key benefits of async/await is a more readable and ergonomic codebase. A goal
of mine for this project was to try to learn readable Rust idioms and create a simple
tool that would act as a relatively easy to understand example. This has some conflicts
with maximum speed -- for example, just using a multithreaded executor like `juliex` would
produce a simpler library. These kinds of tradeoffs are handled on a case by case basis.


## Examples
Send an http request through with default settings. Defaults to no rate limit, no timeouts, `threads` to
`num_cores`, and `connections` to 100.
```
cat tests/GET | clobber --target=0.0.0.0:8000
```

Set a specific duration
```
cat tests/GET | clobber -t 0.0.0.0:8000 -d 2m30s
```

Tweak threads and max connections:
```
cat tests/GET | clobber --target=0.0.0.0:8000 --threads=4 --connections=10000
```

## Tuning TCP for maximum performance

### 1. File/port limits

A common cause of TCP throughput issues is number of open files. You can check this with `ulimit -n`. If you're seeing
issues with number of open files you can raise this limit  with `ulimit` and by editing the `/etc/security/limits.conf`
file. If you're running into too many open ports you have fewer options, but should consider reducing the number of
max connections.

### 2. Connection timeouts

The initial syn phase in the TCP handshake has a long timeout; often in the hundreds of seconds. This is controlled
in `/proc/sys/net/ipv4/tcp_syn_retries`, but even if you set this to a low number a single timeout can take a long
time. This mostly isn't an issue with the intended use case of testing locally running servers with `clobber`, but
if your handshake is unreliable you can try configuring the `connect-timeout` option.

### 3. Read timeouts

Knowing when to stop reading from a TCP stream is tricky if you don't know how much data you should read. This is
protocol dependent, and `clobber` has no idea. If the server doesn't send an `EOF` you can get stuck waiting for more
data for a long time, and this can block connections. With some protocols, such as HTTP, you can send a header like
`Connection: close` that signals to the host that you won't be sending any more requests, and that they should send
an `EOF` after they've responded. This can be important for achieving good throughput. If this isn't possible you
should configure the `read-timeout`, but this does have a bit of an impact on performance (especially with a high
number of connections.)

## Known issues

**Precise Rate Limiting**: A note on timing is that different architectures have different available precision for
sleep timing. A result of this is that you can get a significant increase in performance by not having any rate limit.
For example, if you're trying to hit 20,000 requests per second and only seeing 17k, you may end up getting 25k (or
even quite a bit more) if you fully remove the limit.
