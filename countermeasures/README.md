# Countermeasures

1. [Implementation Strategies](#implementation-strategies)
    1. [Location](#location)
        1. [Client only](#client-only)
        2. [Server Only](#server-only)
        3. [Both](#both)
2. [Defense Strategies](#defense-strategies)
3. [Evaluation](#evaluation)

## Implementation Strategies

### Location

There are three options, where the countermeasures could be implemented:

1. Client
2. Server
3. Both

#### Client only

* Not dependend on any server.
* Covers timing sidechannel on upstream.
* Overhead low as client only talks to one or two servers a time.
* Upstream and downstream correlate.

#### Server Only

* A single implementation protects all clients connecting to the server.
    Likely more than those installing extra tools locally.
* Overhead could be prohibitive if the server serves many clients.
* Upstream still unprotected.

#### Both

* Cooperative strategies might be hard to deploy.
* Two individually implementable strategies (neither client nor server depends on the other one to have the implementation) easier to deploy.
* Can break correlation between upstream and downstream.
* Double countermeasures without cooperation might add twice the delay and bandwidth overhead.

## Defense Strategies

## Evaluation
