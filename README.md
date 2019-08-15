# clobber

`clobber` is a simple TCP load testing tool, written in Rust. It uses the `async/await` syntax, which currently requires the nightly branch, but is targeted to stabilize in the `1.38` release. This project was created as a way to kick the tires of the new syntax, since a network I/O heavy tool is a great use case for an async concurrency model.

## Example

```
echo "GET / HTTP/1.1\r\nHost: localhost:8000\r\n\r\n" | clobber --target=0.0.0.0:8000
```

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

## Design Goals

### 1. Fast

A more efficient TCP client means fewer hosts required to perform a load test. Faster is better for this tool. We try a couple of strategies to try to keep traffic flowing.

#### - Limit open ports and files

Two of the key limiting factors for high TCP client throughput are running out of ports, or opening more files than the underlying OS will allow. `clobber` tries to minimize issues here by giving users control over the max connections. (It's also a good idea to check out your specific `ulimit -n` settings and raise the max number of open files.)

#### - No cross-thread communication
This library uses no cross-thread communication via `std::sync` or `crossbeam`. All futures are executed on a `LocalPool`, and the number of OS threads used is user configurable. This has a number of design impacts. For example, it becomes more difficult to aggregate what each connection is doing. This is simple if you just pass the results to a channel, but this has a non-trivial impact on performance.

*Note: This is currently violated by the way we accomplish rate limiting, which relies on a global thread that manages timers. This ends up putting disproportionate load on that thread at some point. But if you're relying on rate limiting you're trying to slow it down, so we're putting this in the 'feature' column. (If anyone would like to contribute a thread-local futures timer it'd be a great contribution to the Rust community!*)

### 2. Easy to use

It can be a lot of work setting up a load test. `clobber` aims to simply throw a lot of traffic at a host, and much of the time that's all you need. If you need more configuration check out the examples.

## Tips: Tuning and Troubleshooting TCP Performance

There are a couple of small tweaks you can do to the client host to enable much higher throughput.

### 1. File/port limits

A common cause of TCP throughput issues is number of open files. You can check this with `ulimit -n`. If you're seeing
issues with number of open files you can raise this limit with `ulimit`, and by editing the `/etc/security/limits.conf`
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
an `EOF` after they've responded. This can fix throughput issues against some HTTP servers. If this isn't possible you
should configure the `read-timeout`, but this does have a bit of an impact on performance (especially with a high
number of connections.)

## Known issues

**Precise Rate Limiting**: A note on timing is that different architectures have different available precision for
sleep timing. A result of this is that you can get a significant increase in performance by not having any rate limit.
For example, if you're trying to hit 20,000 requests per second and only seeing 17k, you may end up getting 25k (or
even quite a bit more) if you fully remove the limit.
