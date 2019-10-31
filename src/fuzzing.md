# API Fuzzing

Fuzzing is the practice of generating semi-random input and repeatedly submitting it to an interface. 
This is a much more common practice among binary file format applications (think .JPG parsers, .ZIP, etc...), 
and tends to involve compiling the application with a purpose-built compiler that adds coverage hooks to monitor
the program's execution. This is called a "coverage guided" fuzzer, and it allows the fuzzer to immediately know when
it has identified a novel execution path. (Imagine that you have an if/else -- the fuzzer can tell when it hits either
branch.) This means that even if the application's output seems exactly the same the fuzzer can attempt to work its way
through each part of the code and identify novel inputs. 

This works very well for C programs that can be compiled with something like afl-gcc, or clang with LibFuzzer. But what 
about complicated web applications that consist of multiple processes? What if those processes are written in different
languages, or even run on different machines communicating via network calls? This kind of environment is common within
distributed systems, and it makes fuzzing much more difficult. These applications often cannot be compiled with a
fuzzing specific compiler, which leads software teams to simple avoid the process.

`clobber` aims to deliver a black-box fuzzing approach where users provide a starting input (or "seed"), which is then
repeatedly mutated to attempt to find novel input. The goal is for this to provide a fuzzer with minimal setup costs
to enable more teams to fuzz their software. The key challenge here is identifying novel input. Lacking coverage 
guiding means that if two inputs produce identical outputs the fuzzer may not be able to tell that it caused something
interesting to happen. 

## Components 

### clobber 
`clobber` is a TCP load testing tool that simply takes bytes and writes them to a TCP endpoint as fast as possible. 
This is the overall framework that combines the other components to create a configurable testing suite. 
Its intention is to be run locally by developers against a debug application to understand the scaling and performance 
characteristics of the application, or as part of a deployment pipeline to fuzz new releases. This tool should offer
a combination of load/stress testing and fuzzing to understand how your application performs under high load with 
semi-random input.

### byte-mutator
`byte-mutator` is a library for defining a set of rules by which to mutate input. It allows users to define a series
of mutations of different sections of the input, which allows for mutating only the parts of the input that you want 
to test. For example, you might need to compute a checksum or hash of the input -- with `byte-mutator` you can define 
that logic as a stage in the mutation, or just avoid mutating that area if you have something like a key or other 
magic string. 

### response-analyzer 
`response-analyzer` is a library for defining rules to identify novel output from an application. For example, if you
are going to test an HTTP application you may want to consider that all 500 responses ("faults") are unwanted and thus
novel, and attempt to use `clobber` to enumerate areas where your users can cause faults. You may also want to closely
monitor execution time -- if external input can cause your application to burn CPU it opens you up to the possibility
of a denial of service (DOS) from crafted input, even at relatively low traffic volumes. 

Let's be clear here -- response analysis is the most difficult part of this kind of black-box testing. You just don't 
have the kind of insight into what the program is doing if you aren't using a true coverage-guided fuzzer, and I expect
that this will never quite have the results of a solid coverage guided tool like AFL. The balance I want to strike is 
offering _some_ of the benefits of other fuzzers at a fraction of the setup cost, and to a much wider audience as the 
tool is language-agnostic.  