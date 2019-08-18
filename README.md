# clobber

`clobber` is a simple TCP load testing tool written in Rust. It can be a lot of work setting up a load test. `clobber` aims to simply throw a lot of traffic at a host without needing a lot of configuration. 

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
        --repeat     Reuses connections to send the message repeatedly (keep-alive)
    -v               Sets the log level, from -v to -vvv
    -V, --version    Prints version information

OPTIONS:
        --connect-timeout <connect-timeout>    Timeout for initial TCP syn timeout
    -c, --connections <connections>            Max number of open connections at any given time
    -d, --duration <duration>                  Length of the run (e.g. 5s, 10m, 2h, etc...)
    -l, --limit <limit>                        Limit total number of requests
        --rate <rate>                          Limit to a particular rate per-second.
        --read-timeout <read-timeout>          Timeout for reading response from target
    -t, --target <target>                      Host to clobber
        --threads <threads>                    Number of threads

```

## Notes 

### Todo
- [ ] Repeat mode 
- [ ] Input from file path
- [ ] Add back optional timeouts
- [ ] Summary (log analyzer?) 

### Development
`clobber` uses the `async/await` syntax, which currently requires the nightly branch, but is targeted to stabilize in the `1.38` release. This project was created as a way to kick the tires of the new syntax, since a network I/O heavy tool is a great use case for an async concurrency model. 

### Troubleshooting TCP Performance

There are a couple of tweaks you can do to enable much higher throughput.

#### Open file limits

A common cause of TCP throughput issues is number of open files. You can check this with `ulimit -n`. If you're seeing
issues with number of open files you can raise this limit with `ulimit`, and by editing the `/etc/security/limits.conf`
file if the hard limit in `ulimit` is smaller than it needs to be. 

#### Connection timeouts

The initial syn phase in the TCP handshake has a long timeout; often in the hundreds of seconds. This is controlled
in `/proc/sys/net/ipv4/tcp_syn_retries`, but even if you set this to a low number a single timeout can take a long
time. This mostly isn't an issue with the intended use case of testing locally running servers with `clobber`, but
if your handshake is unreliable you can try configuring the `connect-timeout` option.

#### Read timeouts

Knowing when to stop reading from a TCP stream is tricky if you don't know how much data you should read. This is
protocol dependent, and `clobber` has no idea. If the server doesn't send an `EOF` you can get stuck waiting for more
data for a long time, and this can block connections. With some protocols (such as HTTP) you can send a header like
`Connection: close` that signals to the host that you won't be sending any more requests, and that they should send
an `EOF` after they've responded. This can fix throughput issues against some HTTP servers. If this isn't possible you
should configure the `read-timeout`, but this does have a bit of an impact on top-end performance (especially with a high number of connections.)
