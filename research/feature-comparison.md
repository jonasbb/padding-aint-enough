# Feature Comparison for DNS over TLS

1. [DNS Clients](#dns-clients)
2. [DNS Servers](#dns-servers)
3. [DNS Public Resolvers](#dns-public-resolvers)
4. [Padding Schemes](#padding-schemes)

<!--
☑️
❎
✅
❌
-->

Many of the software products and open resolvers are taken from the overview documents of the DNS Privacy project.

There is a WIP IETF draft of the DNS Privacy group to specify padding policies at [draft-ietf-dprive-padding-policy][].

[draft-ietf-dprive-padding-policy]: https://tools.ietf.org/html/draft-ietf-dprive-padding-policy-04

## DNS Clients

| Name                  | TLS   | TLS Libary                   | Authentication     | Request Padding                                    | [Padding Variants](#padding-schemes) | Notes |
| :-------------------- | :---: | ---------------------------- | ------------------ | -------------------------------------------------- | ------------------------------------ | ----- |
| Android P+            | ☑️    | [BoringSSL](Android DNS SSL) |                    | ☑️ [Commit][Bionic request padding]                | blk(128)                             |       |
| Bind (9.12)           | ❎     | n/a                          |                    | ☑️ [`padding` Option][Bind request padding]        | blk(x) with x <= 512                 |       |
| Knot Resolver (2.3.0) | ☑️    | GnuTLS                       | Certificate or Pin | ❎ [#303][Knot Res #303]                            | n/a                                  | 1, 2  |
| Stubby (0.2.2)        | ☑️    | [OpenSSL][Stubby OpenSSL]    | Certificate or Pin | ☑️ [`tls_query_padding_blocksize`][Stubby padding] | blk(x)                               | 3     |
| Unbound (1.7.0)       | ☑️    | OpenSSL                      | Certificate        | ❎                                                  | n/a                                  |       |

1. TLS Forwarding does not allow fallback to UDP or TCP
2. Allows for insecure mode
3. Optional strict mode

[#]: # (Base for all the DNS over TLS forwarding in Android)
[#]: https://android.googlesource.com/platform/system/netd/+/master/server/dns/

## DNS Servers

| Name                  | TLS   | TLS Library | Response Padding                                      | [Padding Variants](#padding-schemes) |
| :-------------------- | :---: | ----------- | ----------------------------------------------------- | ------------------------------------ |
| Bind (9.12)           | ❎     | n/a         | ☑️ [`response-padding` Option][Bind response padding] | blk(x) with x <= 512                 |
| Knot Resolver (2.3.0) | ☑️    | GnuTLS      | ️️☑️ [#247][Knot Res #247]                            | [blk(468)][Knot Res Resp Padding]    |
| Unbound (1.7.0)       | ☑️    | OpenSSL     | ❎                                                     | n/a                                  |

[Android DNS SSL]: https://android.googlesource.com/platform/system/netd/+/9d2a53f8b6eb637891a5767ecb1e3e609930c56e/server/dns/DnsTlsSocket.h#22
[Bionic request padding]: https://github.com/aosp-mirror/platform_bionic/commit/27dd91514797a657d79efe3b902a1ff97bcc5546
[Bind response padding]: https://ftp.isc.org/isc/bind9/cur/9.12/doc/arm/Bv9ARM.ch05.html#options
[Bind request padding]: https://ftp.isc.org/isc/bind9/cur/9.12/doc/arm/Bv9ARM.ch05.html#server_statement_definition_and_usage
[Knot Res #247]: https://gitlab.labs.nic.cz/knot/knot-resolver/merge_requests/247
[Knot Res #303]: https://gitlab.labs.nic.cz/knot/knot-resolver/issues/303
[Knot Res Resp Padding]: https://gitlab.labs.nic.cz/knot/knot-resolver/blob/c64274053e3c24fe408b684acd0413214e91b0bc/lib/defines.h#L75
[Stubby OpenSSL]: https://github.com/getdnsapi/stubby/blob/1a6acd642c7dc9a04cf092e1a3837c5636d4b465/README.md#dependencies
[Stubby padding]: https://github.com/getdnsapi/stubby/blob/1a6acd642c7dc9a04cf092e1a3837c5636d4b465/README.md#create-custom-configuration-file

## DNS Public Resolvers

This test was performed with the Knot Resolver (2.3.0) and kdig.
kdig was used for the padded requests, as it supports both sending padding and TLS.
Knot Resolver was used as a TLS forwarder, but it does not pad outgoing requests.

| Name                             | IP                            | Response Padding (Padded Request) | Response Padding (Unpadded Request) | [Padding Variants](#padding-schemes) |
| :------------------------------- | ----------------------------: | :-------------------------------: | :---------------------------------: | ------------------------------------ |
| Cloudflare                       | `1.1.1.1` / `1.0.0.1`         | ☑️                                | ☑️                                  | blk(468)                             |
| getdnsapi.net                    | `185.49.141.37`               | ❎                                 | ❎                                   | n/a                                  |
| Surfnet (dnsovertls.sinodun.com) | `145.100.185.15`              | ☑️                                | ❎                                   | blk(468)                             |
| Quad9 (secure)                   | `9.9.9.9` / `149.112.112.112` | ❎                                 | ❎                                   | n/a                                  |
| Quad9 (insecure)                 | `9.9.9.10` / `149.112.112.10` | ❎                                 | ❎                                   | n/a                                  |

[DNS Privacy Clients]: https://dnsprivacy.org/wiki/display/DP/DNS+Privacy+Clients
[DNS Privacy Servers]: https://dnsprivacy.org/wiki/display/DP/DNS+Privacy+Test+Servers

## Padding Schemes

The padding schemes are based on the presentation ["Empirical DNS Padding Policy"][Empirical DNS Padding Policy] by Daniel Kahn Gillmor at NDSS DNS Privacy Workshop 2017.

[Empirical DNS Padding Policy]: https://dns.cmrg.net/ndss2017-dprive-empirical-DNS-traffic-size.pdf

| Name                  | Description                                                                                         |
| --------------------- | --------------------------------------------------------------------------------------------------- |
| `blk(sz[, min])`      | pad to blocks of size `sz`, starting at `min`                                                       |
| `copyq`               | pad responses by the amount of query padding                                                        |
| `max`                 | pad queries to 1500, responses to 4096                                                              |
| `pwr(b[, min])`       | pad to powers of base `b`, starting at `min`                                                        |
| `rnd(sz:blks[, min])` | pad to blocks of size `sz`, starting at `min`, plus up to `blks` extra blocks (uniformly at random) |
