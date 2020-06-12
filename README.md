# `clobber`

This branch is a full rewrite of `clobber`, which is a tcp stress testing tool.

## Why a rewrite?

This project started with the thought, "I wonder how many http requests I can make per second with async rust?"

At the time, async hadn't landed to stable rust, so we were in the wild west. 
I tried out both `async-std` and the async branch of `tokio`, did a bit of tuning, and 
was able to confirm that yep, you can make http requests really, really fast with rust.
I still had to do all of the concurrency loop tuning that you'd do in any language, but relatively naive implementations produced high requests per second (rps) right out of the gate. 

I even had to invest in finding faster targets to test, since many simple web services cap out in the low tens of thousands or even thousands of rps. (Note: `python3 -m http.server` is not a good test subject.) 

As I tinker away on `clobber` I've approached the problem of how to produce the highest numbers from a few angles, and I keep coming back to one problem. 

## How many of the thing do I use?  

I've written this loop every way I can think, with dozens of small variations that matter.
But it doesn't matter whether I use os threads, async workers, futures with an executor, I always end up needing to fiddle with how many of the unit of computation I have. 

It's a fundamental problem in distributed services -- how many workers?
How many threads? What's the size of the pool? 

The maddening thing is that the correct answer to this question also changes based on the environment.
If you add a small amount of latency to a system the correct number of workers suddenly changes, and many systems don't have a way to control for that. 

Look around in the systems that you work on -- you'll find this idea hardcoded all over the place. 
How many connections, how many ports, how big is the buffered channel.
They're all the same thing; attempts to guess how many things are correct.

Sometimes the guesses are very good and rarely need tuning because the environment won't change often, and sometimes they're incorrect and are the single bottleneck for your entire system.

## Control Systems theory

I was introduced to control systems theory by Colm MacCartheigh when I was at AWS.
He has an excellent publically available talk on the subject, and I highly recommend that you watch it. Colm convinced me that there is an entire field of scientific thought out there that is nearly directly applicable to the work today's software engineer does on distributed systems, and that we're mostly ignoring it. 

Control theory is the study of dynamic systems and how they can be controlled. 
If you watch YouTube videos you'll decide it's the study of thermostats;
the most popular example is analyzing the loop necessary for your furnace to achieve the heat you asked for without overshooting.
This isn't an instant process, and the entire time the environment can be changing, so it needs to be self correcting to achieve its goal.

Sound familiar? This goes right back to our "how many of the thing" question. 
How many units of work should the controller apply to the furnace to make your house the right temperature? There are PID controllers all over the world that respond to dynamic conditions to control vehicles, massive industrial systems -- human-eating heavy equipment that must be precise. 

Controllers for these situations are highly studied things, and the thinking behind them has lessons for the way that we build distributed software systems. As I reimplement `clobber` from the ground up, I want to take advantage of those lessons.

## Back to `clobber`

`clobber` is now a library about dynamically tuning worker count to produce the highest measured throughput.
It's a tool for situations when the answer to "how fast" is "as fast as possible", and the correct number of workers may change over time. 
