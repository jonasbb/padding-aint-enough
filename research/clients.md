# Clients

See issue [#33][] for more details.

## Table of Contents

1. [Table of Contents](#table-of-contents)
2. [List of Clients](#list-of-clients)
3. [Android 9](#android-9)
    1. [Padding](#padding)
    2. [Out-of-Order](#out-of-order)
    3. [Pipelining](#pipelining)
    4. [Multiple DNS requests per TCP segment](#multiple-dns-requests-per-tcp-segment)
    5. [Retry](#retry)
4. [Firefox](#firefox)
    1. [Padding](#padding-1)
    2. [Out-of-Order](#out-of-order-1)
    3. [Pipelining](#pipelining-1)
    4. [Multiple DNS requests per TCP segment](#multiple-dns-requests-per-tcp-segment-1)
    5. [Retry](#retry-1)
5. [Intra](#intra)
    1. [Padding](#padding-2)
    2. [Out-of-Order](#out-of-order-2)
    3. [Pipelining](#pipelining-2)
    4. [Multiple DNS requests per TCP segment](#multiple-dns-requests-per-tcp-segment-2)
    5. [Retry](#retry-2)
6. [Stubby](#stubby)
    1. [Padding](#padding-3)
    2. [Out-of-Order](#out-of-order-3)
    3. [Pipelining](#pipelining-3)
    4. [Multiple DNS requests per TCP segment](#multiple-dns-requests-per-tcp-segment-3)
    5. [Retry](#retry-3)
7. [Specification](#specification)
    1. [Padding](#padding-4)
    2. [Retry](#retry-4)

## List of Clients

* [Android 9][]: DoT
* [Cloudflare App][]: ?, probably DoT
* [Firefox][]: DoH
* [Intra][]: DoH
* [Stubby][]: DoT

## Android 9

The analysis refers to commit [`16b6d76f0ebb8d355f95fe65c1a110117c05e1c3`][] in the `platform/system/netd` repository.

### Padding

Uses the 128B client query padding.
See [lines 89-90](https://android.googlesource.com/platform/system/netd/+/16b6d76f0ebb8d355f95fe65c1a110117c05e1c3/resolv/res_mkquery.cpp#89) and [lines 234-249](https://android.googlesource.com/platform/system/netd/+/16b6d76f0ebb8d355f95fe65c1a110117c05e1c3/resolv/res_mkquery.cpp#234) in `resolv/res_mkquery.cpp`.

### Out-of-Order

The implementation tries to send as soon as possible.
The implementation uses sockets and poll() to communicate.
The Bionic stub resolver spawns one thread per request.

As such, there are multiple source which would allow out-of-order processing.

### Pipelining

Yes.

### Multiple DNS requests per TCP segment

Unknown

### Retry

The file `server/dns/README.md` specifies:

> `DnsTlsSocket` imposes a 20-second inactivity timeout.  A socket that has been idle for
> 20 seconds will be closed.  This sets the limit of tolerance for slow replies,
> which could happen as a result of malfunctioning authoritative DNS servers.
> If there are any pending queries, `DnsTlsTransport` will retry them.
>
> `DnsTlsQueryMap` imposes a retry limit of 3.  `DnsTlsTransport` will retry the query up
> to 3 times before reporting failure to `DnsTlsDispatcher`.
> This limit helps to ensure proper functioning in the case of a recursive resolver that
> is malfunctioning or is flooded with requests that are stalled due to malfunctioning
> authoritative servers.

## Firefox

[Source Code](https://github.com/mozilla/gecko/tree/3f502e430c7887baaca10a1246a265bd4d51e187/netwerk/dns)

Firefox allows servers to push DNS data [`netwerk/dns/TRR.cpp:464`][].

### Padding

Unknown.
No reference to padding found in the source code or by searching the [Bugzilla](https://bugzilla.mozilla.org/).

### Out-of-Order

Unclear

### Pipelining

Unclear, probably yes given the HTTP transport.

### Multiple DNS requests per TCP segment

Unclear

### Retry

There is a retry mechanism in the code, as this is needed for the fallback to UDP.
However, it is unclear if TRR only mode has a retry timer.

## Intra

### Padding

No.
See [issue #98][Intra #98].

### Out-of-Order

### Pipelining

### Multiple DNS requests per TCP segment

Possible.
See [issue #80][Intra #80].

### Retry

Unclear.
There is a retry test, which test if a unresponsible server becomse responsible, if the size is reduced.
But no explicit mentions of query retries.

## Stubby

The analysis refers to commit [`0964c357d574a66a9cacb53a908cdcc27428819b`].

### Padding

Yes.
Default to 128B block size, see [`stubby.yml.example:59`][].

[`stubby.yml.example:59`]: https://github.com/getdnsapi/stubby/blob/0964c357d574a66a9cacb53a908cdcc27428819b/stubby.yml.example#L56-L59
[`0964c357d574a66a9cacb53a908cdcc27428819b`]: https://github.com/getdnsapi/stubby/blob/0964c357d574a66a9cacb53a908cdcc27428819b/

### Out-of-Order

### Pipelining

### Multiple DNS requests per TCP segment

It does not seem to use `TCP_NODELAY`.

### Retry

No mention of retries.

## Specification

### Padding

Keywords: padding, EDNS0, RFC 7830 (Padding Option), RFC 8467 (Padding Policies)

**DoH**:

[RFC 8484 Section 4.1][RFC8484-Sec4.1]:
> DoH clients can use HTTP/2 padding and compression [RFC7540] in the
> same way that other HTTP/2 clients use (or don't use) them.

[RFC 8484 Section 8.1][RFC8484-Sec8.1]:
> DoH encrypts DNS traffic and requires authentication of the server.
> This mitigates both passive surveillance [RFC7258] and active attacks
> that attempt to divert DNS traffic to rogue servers (see
> Section 2.5.1 of [RFC7626]).  DNS over TLS [RFC7858] provides similar
> protections, while direct UDP- and TCP-based transports are
> vulnerable to this class of attack.  An experimental effort to offer
> guidance on choosing the padding length can be found in [RFC8467].

[RFC 8484 Section 9][RFC8484-Sec9]:
> […] HTTP/2 provides further advice about the use of
> compression (see Section 10.6 of [RFC7540]) and padding (see
> Section 10.7 of [RFC7540]).  DoH servers can also add DNS padding
> [RFC7830] if the DoH client requests it in the DNS query.  An
> experimental effort to offer guidance on choosing the padding length
> can be found in [RFC8467].

### Retry

[RFC 7858 Section 3.4][RFC7858-Sec3.4] specifies:
> Clients and servers that keep idle connections open MUST be robust to
> termination of idle connection by either party.  As with current DNS
> over TCP, DNS servers MAY close the connection at any time (perhaps
> due to resource constraints).  As with current DNS over TCP, clients
> MUST handle abrupt closes and be prepared to reestablish connections
> and/or retry queries.

[#33]: https://projects.cispa.saarland/bushart/encrypted-dns/issues/33
[`16b6d76f0ebb8d355f95fe65c1a110117c05e1c3`]: https://android.googlesource.com/platform/system/netd/+/16b6d76f0ebb8d355f95fe65c1a110117c05e1c3/
[`netwerk/dns/TRR.cpp:464`]:(https://github.com/mozilla/gecko/tree/3f502e430c7887baaca10a1246a265bd4d51e187/netwerk/dns/TRR.cpp#L464)
[Android 9]: https://android-developers.googleblog.com/2018/04/dns-over-tls-support-in-android-p.html
[Cloudflare App]: https://blog.cloudflare.com/1-thing-you-can-do-to-make-your-internet-safer-and-faster/
[Firefox]: https://blog.nightly.mozilla.org/2018/06/01/improving-dns-privacy-in-firefox/
[Intra #80]: https://github.com/Jigsaw-Code/Intra/issues/80
[Intra #98]: https://github.com/Jigsaw-Code/Intra/issues/98
[Intra]: https://github.com/Jigsaw-Code/Intra
[RFC7858-Sec3.4]: https://tools.ietf.org/html/rfc7858#section-3.4
[RFC8484-Sec4.1]: https://tools.ietf.org/html/rfc8484#section-4.1
[RFC8484-Sec8.1]: https://tools.ietf.org/html/rfc8484#section-8.1
[RFC8484-Sec9]: https://tools.ietf.org/html/rfc8484#section-9
[Stubby]: https://dnsprivacy.org/wiki/display/DP/DNS+Privacy+Daemon+-+Stubby
