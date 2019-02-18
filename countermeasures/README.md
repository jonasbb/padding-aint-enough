# Countermeasures

1. [Implementation Strategies](#implementation-strategies)
    1. [Location](#location)
2. [Defense Strategies](#defense-strategies)
3. [Evaluation](#evaluation)
4. [References](#references)

## Implementation Strategies

There are different approaches to implementing the countermeasures.
We can take existing software and modify it, to include this behavior.
We can build on existing libraries and implement our own client/server.
Lastly, we could work on the raw network traffic and build a tool which alters the traffic.

Extending or modifying existing software is likely rather difficult and time consuming.
It does not scale well, so tests with different software are unlikely.
Measuring the overhead here would provide better results, as we might take advantage of the software to schedule future packets, without storing them in memory.
We also have access to the internal state, allowing us to implement padding with pre-fetching.

Building our own software has similar trade-off as before.
It might be easier to write, given a good library.
This mostly holds for writing a stub resolver, not so much for the DNS resolver, as these are massivly complex.
This approach would give us the most freedome.

We are mostly interested in altering the network characteristics and evaluating the performance impact.
As such, it is not strictly necessary to have a DNS client/server, but delaying network packets can achive the same benefits.
This approach would be software agnostic and works on both the client and server.
We have no integration points into the DNS software, thus have no synergies there which could lower the overhead.
For example at the resolver, instead of storing outgoing packets in memory, we could delay the sending by adding an asynchronous sleep, thus lowering the memory overhead.

### Location

There are three options, where the countermeasures could be implemented:

* **Client Only**
    * Not dependend on any server.
    * Covers timing sidechannel on upstream.
    * Overhead low as client only talks to one or two servers a time.
    * Upstream and downstream correlate.
* **Server Only**
    * A single implementation protects all clients connecting to the server.
        Likely more than those installing extra tools locally.
    * Overhead could be prohibitive if the server serves many clients.
    * Upstream still unprotected.
* **Both**
    * Cooperative strategies might be hard to deploy.
    * Two individually implementable strategies (neither client nor server depends on the other one to have the implementation) easier to deploy.
    * Can break correlation between upstream and downstream.
    * Double countermeasures without cooperation might add twice the delay and bandwidth overhead.

## Defense Strategies

What are the most promosing strategies for removing timing sidechannels?

* **constant-rate:**
    The send rate is set to a fixes value of every *x* ms.
    This is the strongest setting, as now no timing sidechannel can exist, since the time is fixed to *x*.
    There is a bandwidth overhead problem, in that every *x* ms a packet **must** be send out.
    Also, a latency overhead, as even with empty buffers, payload on average has to wait *x/2* ms before being transmitted.
    This only gets worse if buffers start filling up.
* **adaptive padding (AP):**
    AP is a state machine, which switches between idle, burst mode, and gap mode.
    In burst mode and gap mode, time values are sampled from a distribution and traffic (mostly) adheres to them.
    Real traffic is always send immediatly, thus minimizing overall latency.
    The WTF-Pad paper describes adaptive padding and their own improvements of it.
    It has to be seen how necessary a burst mode is, as this basically does not happen for DNS.
* **superset morphing:**
    Assuming we have a sequence *s* and a super-sequence *sup* with the following properties.

    * |*s*| < |*sup*|
    * There is an injective mapping from every element of *s* to an element of *sup*, such that the sum of gaps before the element is smaller in *s* than in *sup*.
        What this give us is the option to delay each element of *s* such that it matches up with some later element of *sup*.

    Then we can transform the lookup sequence *s* into *sup* by adding delays and sending dummy messages according to the schedule *sup*.

    A problem is identifying the start of *s* and at the same time knowing what a suitable *sup* would be.
* **padding:**
    Other ways of padding can be explored, although we already measured that this likely only has a small impact.
    We might want to consider the impact DNSSEC has here and adjust the recommendation based on this.

## Evaluation

* Latency of DNS requests impacted?

    Measure the time until the response for a DNS query is received.
* Website load time?

    Measure the load time of a website.
* Bandwidth consumption?

    How many more packets are transmitted due to the countermeasure?
* Does it work?

    What is the performance of the classifier, potentially adopted to the countermeasure, compared to the original?

## References

* WTF-PAD: Toward an Efficient Website Fingerprinting Defense for Tor
