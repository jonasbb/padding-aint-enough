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

## DNS Clients

| Name                  | TLS   | TLS Libary                | Authentication     | Request Padding                                    | [Padding Variants](#padding-schemes) | Notes |
| :-------------------- | :---: | ------------------------- | ------------------ | -------------------------------------------------- | ------------------------------------ | ----- |
| Android P+            |       |                           |                    |                                                    |                                      |       |
| Bind (9.12)           | ❎     | n/a                       |                    | ☑️ [`padding` Option][Bind request padding]        | blk(x) with x <= 512                 |       |
| Knot Resolver (2.3.0) | ☑️    | GnuTLS                    | Certificate or Pin | ❎ [#303][Knot Res #303]                            | n/a                                  | 1, 2  |
| Stubby (0.2.2)        | ☑️    | [OpenSSL][Stubby OpenSSL] | Certificate or Pin | ☑️ [`tls_query_padding_blocksize`][Stubby padding] | blk(x)                               | 3     |
| Unbound (1.7.0)       | ☑️    | OpenSSL                   | Certificate        | ❎                                                  | n/a                                  |       |

1. TLS Forwarding does not allow fallback to UDP or TCP
2. Allows for insecure mode
3. Optional strict mode

## DNS Servers

| Name                  | TLS   | TLS Library | Response Padding                                      | [Padding Variants](#padding-schemes) |
| :-------------------- | :---: | ----------- | ----------------------------------------------------- | ------------------------------------ |
| Bind (9.12)           | ❎     | n/a         | ☑️ [`response-padding` Option][Bind response padding] | blk(x) with x <= 512                 |
| Knot Resolver (2.3.0) | ☑️    | GnuTLS      | ️️☑️ [#247][Knot Res #247]                            | [blk(468)][Knot Res Resp Padding]    |
| Unbound (1.7.0)       | ☑️    | OpenSSL     | ❎                                                     | n/a                                  |

[Bind response padding]: https://ftp.isc.org/isc/bind9/cur/9.12/doc/arm/Bv9ARM.ch05.html#options
[Bind request padding]: https://ftp.isc.org/isc/bind9/cur/9.12/doc/arm/Bv9ARM.ch05.html#server_statement_definition_and_usage
[Knot Res #247]: https://gitlab.labs.nic.cz/knot/knot-resolver/merge_requests/247
[Knot Res #303]: https://gitlab.labs.nic.cz/knot/knot-resolver/issues/303
[Knot Res Resp Padding]: https://gitlab.labs.nic.cz/knot/knot-resolver/blob/c64274053e3c24fe408b684acd0413214e91b0bc/lib/defines.h#L75
[Stubby OpenSSL]: https://github.com/getdnsapi/stubby/blob/1a6acd642c7dc9a04cf092e1a3837c5636d4b465/README.md#dependencies
[Stubby padding]: https://github.com/getdnsapi/stubby/blob/1a6acd642c7dc9a04cf092e1a3837c5636d4b465/README.md#create-custom-configuration-file

## DNS Public Resolvers

Tested with Knot Resolver, which itself does not pad. This might affect hte results.

| Name                             | Response Padding | [Padding Variants](#padding-schemes) |
| :------------------------------- | :--------------: | ------------------------------------ |
| Cloudflare                       | ☑️               | blk(468)                             |
| getdnsapi.net                    | ❎                | n/a                                  |
| Surfnet (dnsovertls.sinodun.com) | ❎                | n/a                                  |
| Quad9 (secure)                   | ❎                | n/a                                  |
| Quad9 (insecure)                 | ❎                | n/a                                  |

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
