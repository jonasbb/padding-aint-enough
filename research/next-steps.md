# Next Steps

1. [Current State](#current-state)
    1. [Dataset Size](#dataset-size)
2. [Improving the Dataset](#improving-the-dataset)
3. [Improving the Results](#improving-the-results)
    1. [Examining different `k`'s and Tie-Breakers](#examining-different-ks-and-tie-breakers)
4. [Measuring Privacy Impact](#measuring-privacy-impact)
    1. [Extract DNS Message Size](#extract-dns-message-size)
    2. [Padding Options](#padding-options)
    3. [Anonymity of a Single Query/Response](#anonymity-of-a-single-queryresponse)
5. [Missing Research](#missing-research)
    1. [Traffic Traces over Time](#traffic-traces-over-time)
    2. [Different Clients](#different-clients)
    3. [Impact of larger Datasets](#impact-of-larger-datasets)
    4. [With or Without DNSSEC](#with-or-without-dnssec)
    5. [TBD](#tbd)
6. [Limitations / Assumptions](#limitations--assumptions)

## Current State

There are two datasets of DNS traces (as dnstap files) and Chrome debugger message logs for Alexa top 10k scans.
One dataset is with a DNSSEC validating resolver, the other one performs no DNSSEC.

The DNSSEC dataset is not perfect as it seems to contain DNSSEC validation failures for at least the `.de`-TLD.

### Dataset Size

The current dataset is build from the Alexa top 10k, where each URL is fetched 10 times.

Other traffic analysis paper often use a set of monitored domains in the lower thousands.
They may add additional open-world domains (in the order of 1000s to 100000s) with a single trace usually.

The current dataset size seems sufficiently large.

## Improving the Dataset

A re-run with additional cache updating logic might reduce or eliminate the DNSSEC validation errors.

## Improving the Results

Right now the edit distance uses some guessed constants.
Different constants might lead to better results and are worth exploring.

The contants so far are:

|         | Insert: Size | Insert: Gap | Subs.: S -> S       | Subs.:  S -> G | Subs.:  G -> G | Subs.:  G -> S | Swap |
| ------- | -----------: | ----------- | ------------------- | -------------- | -------------- | -------------- | ---: |
| Initial | 20           | Gap * 5     | (Insert+Delete) / 3 | Insert+Delete  | abs(G1-G2) * 2 | Insert+Delete  | 20   |

Another factor is the conversion of a time delta into the gap value.
TODO

### Examining different `k`'s and Tie-Breakers

Different values for `k` in knn can be chose.
However, it seems that `k=1` is very hard to beat in any measurement.
See this [comparison chart](https://projects.cispa.saarland/bushart/encrypted-dns/snippets/37).

Similarly, the different tie breakers each improve performance, however, none reaches a better performance than `k=1` and for `k=1` all tie breakers are identical.
It seems choosing a good tie breaker is not that important.

## Measuring Privacy Impact

### Extract DNS Message Size

The current assumption is, that it is possible to extract the DNS message size from the observed TCP segments.
This is possible for DoT, as this only adds the TLS overhead + 2 Bytes message length.
However, it is necessary that the DNS queries/responses are sent in different TCP segments.
This was the case so far, but it does not have to be this way.
Pipelining is explicitly allowed and out of order responses as well.
There is likely a performance benefit, in terms of latency, by using one segment per DNS message, thus it is not unlikely that this behaviour will persist.

It is unclear right now how this works for DNS over HTTP.
HTTP/2 can have message padding which is independent of the DNS padding.
There are two different ways to encode a DNS query, either as base64 GET parameter or in UDP wire format in a POST.

Additionally, different HTTP headers can be set by client and by server, making it difficult to infer the DNS message size by just the TCP segment size.
Information, such as the IP address of server, might help, as this could allow measuring the sent HTTP headers by the server and use this for infering the DNS message size.

### Padding Options

Different padding schemes will result in different overhead and privacy guarantees.
A list of different padding schemes can be found [here](./feature-comparison.md#padding-schemes).

### Anonymity of a Single Query/Response

Prior work analyzed the Anonymity of a single query/response pair by looking into the k-anonymity factor of queries and responses.
We can try to reproduce those results with the current Alexa DNS traces.
We can extract all the traces, apply different padding options, and recreate the measurements from the presentation.

The presentation defines two costs:

* $\beta$ - Bandwidth cost
    * Cost to defenders
    * Add up padded sizes, normalize by unpadded size
    * $\beta = \frac{\sum_{x,y}{(x+y)P_{x,y}}}{\sum_{x,y}{(x+y)U_{x,y}}}$
    * Best: 1, higher values are worse
* $\phi$ - Followup cost
    * Cost to attacker
    * Attacker is interested in **one particular Q/R pair**
    * Attacker only sees padded sizes
    * How many other Q/R pairs could be mixed in with the target?
    * $\phi = \frac{\sum_{i,j,x,y|T_{i,j \rightarrow x,y} > 0}{(U_{i,j}P_{x,y}})}{N^2}$
    * Best: 1, lower values are worse

See [Empirical DNS Padding Policy] for details.

[Empirical DNS Padding Policy]: https://dns.cmrg.net/ndss2017-dprive-empirical-DNS-traffic-size.pdf

## Missing Research

### Traffic Traces over Time

How stable are the traffic traces?
Measure a subset of the 10000 domains with a quicker interval and see how the privacy impact changes.

### Different Clients

Right now the testing environment was with two dedicated VMs.
Do different clients behave the same or similar?
Are the learned traces applicable to other clients, especially if they have different latency and bandwidth characteristics?

### Impact of larger Datasets

How is the performance of the classifier impacted by the size of the dataset?
Does the Alexa ranking have an influence on the success rate?

The intuition says, that larger datasets reduce the performance, as it is "easier" to confuse two domains.

### With or Without DNSSEC

There are different modes, how an upstream resolver can be used.

* Trusted Recursive Resolver (TRR)

    The resolver is fully trusted and has to perform recursive DNS queries.
    This mode is used by Firefox's DoH study.
    All DoT and DoH capable resolvers can be used like this.
* Local DNSSEC validating resolver (Local)

    DNSSEC validation errors are only marked as SERVFAIL.
    It is not always possible to see that an error is caused by DNSSEC.
    Fallback to a second upstream resolver (without DNSSEC) would return a working response.

    Therefore, to ensure that DNSSEC is always validated a local resolver must be run.
    This is the approach taken by Fedora.

If the TRR approach is taken, no DNSSEC related queries should be visible and the only DNSSEC related responses should be `NSEC(3)` (if at all).
With the Local approach lots of DNSSEC related queries occur, for the different `DS`, `DNSKEY`, `NSEC3PARAMS` and many DNSSEC related responses like `RRSIG` and `NSEC(3)`.

### TBD

Create measure of impact.
How likely is an impact on privacy.

Given a trace, how large is the k for k-anonymity.

## Limitations / Assumptions

* The cache is empty except for the TLD's in the resolver.
    If either the browser or DNS cache contains the required data, then no outgoing request will be seeen.
* The client finishes loading the page.
* There are no concurrent DNS requests.
* SNI is not encrypted (pending draft for ESNI), so curious ISP has other options.
