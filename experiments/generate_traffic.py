#!/usr/bin/env python3

from base64 import b64encode
import random
import typing as t

from scapy.layers.dns import DNS, DNSQR


def mkpacket(domain: str) -> bytes:
    dnsid = random.randint(0, 2**16 - 1)
    return DNS(id=dnsid, rd=1, qd=DNSQR(qname=domain)).build()


def main() -> None:
    base_domain = ".bushart.org"
    prefixes = ["a" * i for i in range(1, 60)]
    domains = [p + base_domain for p in prefixes]
    cmd = '''echo -n '{}' | base64 -d | env SSLKEYLOGFILE=tlskeys curl --http1.1 -H 'Content-Type: application/dns-udpwireformat' --data-binary @- https://1.0.0.1/dns-query -o - | hexdump'''
    cmds = [cmd.format(b64encode(mkpacket(domain)).decode('utf8')) for domain in domains]
    with open('./dohdns', 'w') as f:
        f.write('\n'.join(cmds))


if __name__ == '__main__':
    main()
