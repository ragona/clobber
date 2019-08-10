# clobber

`clobber` is a simple TCP load testing tool.

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

## Examples
Send an http request through with default settings:
```
cat tests/GET | clobber --target=0.0.0.0:8000
```

Tweak threads and max connections:
```
cat tests/GET | clobber --target=0.0.0.0:8000 --threads=4 --connections=10000
```

## Tuning TCP for maximum performance

Todo

## Known issues

**Precise Rate Limiting**: A note on timing is that different architectures have different available precision for sleep timing. The result of this is that precisely rate limiting in micro/nanoseconds is unreliable. On my laptop I can precisely limit to around 150k requests per second, but can achieve around 1m when `clobber` has no rate limit.
