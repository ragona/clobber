> :warning: **Project in early development!** This is not a stable library. 

# `clobber`

`clobber` is a library about dynamically tuning concurrent workloads. 
It's a tool for situations when the answer to "how many workers?" isn't obvious, or you expect that the answer will shift as the system's environment changes. 

## How many at once?  

We know making a lot of network requests is a time for concurrency. 
But how many requests should we make at once? 
Threads, futures, async -- all techniques require us to answer this question correctly. 

This is a fundamental problem in distributed services.
How many workers?
How many threads? 
What's the size of the pool? 
There's no perfect answer.
If you set the number too low you'll have low throughput and underutilized hardware.
It you set the number too high the workers will start to contend with each other for some resource or other and you'll waste CPU stepping on your own toes.

Maddeningly, it will also change! The environment has a huge impact. 
If you add a small amount of latency to a system the correct number of workers suddenly changes, and many systems don't have a way to control for that. 
They're just wrong, and they limp along sub-optimally. 

Look around in the systems that you work on -- you'll find this idea hardcoded all over the place. 
How many connections, how many ports, open file descriptors, how big is the buffered channel.
They're all the same thing; attempts to guess how many things to use at once.

Sometimes the guesses are very good and rarely need tuning (usually because the environment won't change often), and sometimes they're incorrect and are the single bottleneck for your entire system.

## Control Systems theory

I was introduced to control systems theory by Colm MacCartheigh when I was at AWS.
He has [multiple](https://www.youtube.com/watch?v=3AxSwCC7I4s) [talks](https://www.youtube.com/watch?v=O8xLxNje30M) and a [twitter thread](https://twitter.com/colmmacc/status/1071089567246114816) on the subject, and I recommend all of them. 

Colm convinced me that there is an entire field of scientific thought out there that is nearly directly applicable to the work today's software engineer does on distributed systems, and that we're mostly ignoring it. 

Control theory is the study of dynamic systems and how they can be controlled. 
If you watch YouTube videos you'll decide it's the study of thermostats;
the most popular example is analyzing the loop necessary for your furnace to achieve the heat you asked for without overshooting.
This isn't an instant process, and the entire time the environment can be changing, so it needs to be self correcting to achieve its goal.

Sound familiar? This goes right back to our "how many of the thing" question. 
How many units of work should the controller apply to the furnace to make your house the right temperature? When should it ease off? 

There are hardware controllers (look up PID controller) all over the world that respond to dynamic conditions to control vehicles, massive industrial systems -- human-eating heavy equipment that must be precise. 

Controllers for these situations are highly studied things, and the thinking behind them has lessons for the way that we build distributed software systems. As I reimplement `clobber` from the ground up, I want to try to use those lessons to answer "how many of the thing".
## Making a PID controller 

I want a PID controller to solve my number of workers problem, so I need to build a PID controller.
To wikipedia! 

![PID Controller](https://upload.wikimedia.org/wikipedia/commons/4/43/PID_en.svg)

Don't worry, we can break this down. 
These kinds of formulas used to freak me out, but after spending enough time working with cryptographers I realized mathmeticans are just programmers who prefer walls to computers.
The poor dears work on chalkboards and haven't the typing speed to use what we would consider polite variable names, so they resort to the greek alphabet to avoid multi-letter variables.
If you had to write all of your variables with chalk you'd use funny letters too.

Each of the three major areas is an independent controller that uses its own strategy to move us towards our goal value. 
Together they make up a PID controller, but we could also use a proportional controller on its own -- it just wouldn't have the integral or derivative functions to shore up its weaknesses.

### Formula notes 
As I start turning this formula into code there are a couple of things to keep in mind that will help it make sense.

#### Time
Notice how we have "t" all over the place? This whole bonkers looking equation is just a for loop. `t` represents time, and the first part of the integral (`0∫t`) declares `for 0 to t`.

We can approach time differently from mathmeticians. 
In pseudocode I'll take time out of the equations and assume that it's a member variable of a struct.
I'll poll this module at some interval and get its recommendation on the correct number of workers. 
Anywhere that has `e(t)` can simply become `e` or `error` for readability.

#### What is with `K` everywhere
This is a constant that allows us to tune the "gain" or strength of each controller.

### Goal (target rps)
```
r(t)
```
Our goal, also sometimes called set point (`SP`). In our case, this is how many rps we want to achieve.

### Current (current rps)
```
y(t)
```
The current state of the system, or process value (`PV`). 

### Output (workers)
- `u(t)`: Output

### Error 
```
e(t)
```
Error is `goal - current`. If we wanted to make 100 requests per second and we just started at zero, then the error is 100.

### Proportional
```
Kp e(t)
```

This is the first and simpliest of our three independent controllers. 
Just by using more human names for the variables and removing time from the equation we get:

```
gain * error
```

For this controller the error is simply `goal - current`. 
If we're hoping to perform a loadtest at 100 rps and our gain is set to 1.0, then as soon as our test starts we'll jump to correct the error and add 100 workers. 
Okay, too aggressive, but the idea makes sense.

### Integral
```
Ki 0∫t e(t)dt
```

Alright, most of the complexity on this one is actually just the math equivalent of curly braces and loop declarations to reason about time. 
But what is that `dt` at the end there? 
[StackExchange](https://math.stackexchange.com/questions/1479059/what-is-the-difference-between-d-dt-and-dy-dt) has us covered.

`dt` is the differential operator; it's notation relating to the fact that we're operating on a data set.
The important thing to remember here is that this controller looks at error **over time**, so we'll need to do that too and add to the error every time instead of recalculating it. 

If we strip all of that we're left with:

```
gain * error 
```

Oh, sure. Same idea as the first one, but this time we track error over time instead of just looking at the current gap.

### Derivative
```
Kd de(t) / dt
```

Where the other controllers look at what the error is or has been, the derivative controller looks at how fast the error is changing and it tries to stop that change. 

```
error_delta = (error - last_error) 
gain * error_delta
```

It's a counterbalance for the other two. 
And it's still pretty much `gain * error`.

Let's go over the example where we'd just started a loadtest with a goal rate of 100 rps.
It's the second tick. 
Our first two data points were 0 and 20, so `error - last_error` is `80 - 100` or `-20`. 
If the derivative gain (`Kd`) is 1.0, then this controller's input will be a reduction in workers by 20.

## Putting it together

```rust
enum ControllerType {
    Proportional,
    Integral,
    Derivative,
}

struct Controller {
    pub controller_type: ControllerType,
    pub gain: f32,
    pub error: f32,
}

impl Controller {
    pub fn new(controller_type: ControllerType, gain: f32) -> Self {
        Self {
            controller_type,
            gain,
            error: 0.0,
        }
    }

    pub fn update(&mut self, error: f32) {
        self.error = match self.controller_type {
            ControllerType::Proportional => error,
            ControllerType::Integral => (error + self.error) / 2.0,
            ControllerType::Derivative => error - self.error,
        }
    }

    pub fn output(&self) -> f32 {
        self.error * self.gain
    }
}

pub struct PidController {
    p: Controller,
    i: Controller,
    d: Controller,
}

impl PidController {
    /// Creates a new PidController with the provided `gain` tuple.
    /// Gain is used to balance the respective volume of each controller.
    pub fn new(gain: (f32, f32, f32)) -> Self {
        let (p_gain, i_gain, d_gain) = gain;
        Self {
            p: Controller::new(ControllerType::Proportional, p_gain),
            i: Controller::new(ControllerType::Integral, i_gain),
            d: Controller::new(ControllerType::Derivative, d_gain),
        }
    }

    pub fn update(&mut self, goal: f32, current: f32) {
        let error = goal - current;

        self.p.update(error);
        self.i.update(error);
        self.d.update(error);
    }

    pub fn output(&self) -> f32 {
        self.p.output() + self.i.output() + self.d.output()
    }
}

```

The liberty I'm taking here is in calling at a fixed interval and simply integrating and deriving one step at a time, which dramatically simplifies each formula.
