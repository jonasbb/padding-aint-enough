# Next Steps

1. [Current State](#current-state)
2. [Improving the Dataset](#improving-the-dataset)
3. [Improving the Results](#improving-the-results)
4. [Measuring Privacy Impact](#measuring-privacy-impact)
    1. [Extract DNS Message Size](#extract-dns-message-size)
    2. [Padding Options](#padding-options)
    3. [Anonymity of a Single Query/Response](#anonymity-of-a-single-queryresponse)
    4. [TBD](#tbd)

## Current State

There are two datasets of DNS traces (as dnstap files) and Chrome debugger message logs for Alexa top 10k scans.
One dataset is with a DNSSEC validating resolver, the other one performs no DNSSEC.

The DNSSEC dataset is not perfect as it seems to contain DNSSEC validation failures for at least the `.de`-TLD.

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
We can extract all the traces, apply different padding options, and then determine the k-anonymity which is achievable.

### TBD

Create measure of impact.
How likely is an impact on privacy.

Given a trace, how large is the k for k-anonymity.
