# ---
# jupyter:
#   jupytext:
#     formats: ipynb,py:percent
#     text_representation:
#       extension: .py
#       format_name: percent
#       format_version: '1.2'
#       jupytext_version: 1.2.0+dev
#   kernelspec:
#     display_name: Python 3
#     language: python
#     name: python3
# ---

# %% [markdown]
# # PCAP to DNSTAP Differences
#
# The list of the 20 most problematic sequences can be found here: https://projects.cispa.saarland/bushart/encrypted-dns/issues/43
# A copy is provided below.

# %% [markdown]
# | Analyzed | Domain                | Distance |                                                                                           Problematic Sequence |
# | :------: | :-------------------- | -------: | -------------------------------------------------------------------------------------------------------------: |
# |    ❌     | allkpop.com           |     1896 |                     /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/allkpop.com/allkpop.com-5-0.dnstap.xz |
# |    ❌     | damndelicious.net     |     2224 |         /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/damndelicious.net/damndelicious.net-2-0.dnstap.xz |
# |    ❌     | damndelicious.net     |     1655 |         /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/damndelicious.net/damndelicious.net-6-0.dnstap.xz |
# |    ❌     | damndelicious.net     |     1838 |         /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/damndelicious.net/damndelicious.net-7-0.dnstap.xz |
# |    ❌     | damndelicious.net     |     2498 |         /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/damndelicious.net/damndelicious.net-8-0.dnstap.xz |
# |    ❌     | realclearpolitics.com |     1812 | /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/realclearpolitics.com/realclearpolitics.com-1-0.dnstap.xz |
# |    ❌     | realgm.com            |     2136 |                       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/realgm.com/realgm.com-1-0.dnstap.xz |
# |    ❌     | realgm.com            |     1641 |                       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/realgm.com/realgm.com-3-0.dnstap.xz |
# |    ❌     | realgm.com            |     1663 |                       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/realgm.com/realgm.com-5-0.dnstap.xz |
# |    ❌     | realgm.com            |     2103 |                       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/realgm.com/realgm.com-9-0.dnstap.xz |
# |    ❌     | thoughtcatalog.com    |     1693 |       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/thoughtcatalog.com/thoughtcatalog.com-2-0.dnstap.xz |
# |    ❌     | thoughtcatalog.com    |     2011 |       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/thoughtcatalog.com/thoughtcatalog.com-5-0.dnstap.xz |
# |    ❌     | thoughtcatalog.com    |     1803 |       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/thoughtcatalog.com/thoughtcatalog.com-7-0.dnstap.xz |
# |    ❌     | thoughtcatalog.com    |     2730 |       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/thoughtcatalog.com/thoughtcatalog.com-8-0.dnstap.xz |
# |    ❌     | wetter.com            |     1760 |                       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/wetter.com/wetter.com-1-0.dnstap.xz |
# |    ❌     | wetter.com            |     1742 |                       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/wetter.com/wetter.com-2-0.dnstap.xz |
# |    ❌     | wetter.com            |     1731 |                       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/wetter.com/wetter.com-3-0.dnstap.xz |
# |    ❌     | wetter.com            |     1859 |                       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/wetter.com/wetter.com-7-0.dnstap.xz |
# |    ❌     | wetter.com            |     1948 |                       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/wetter.com/wetter.com-8-0.dnstap.xz |
# |    ❌     | wetter.com            |     1706 |                       /mnt/data/Downloads/dnscaptures-2019-09-06/extracted/wetter.com/wetter.com-9-0.dnstap.xz |

# %%
import json
from pprint import pprint

import pylib

# %%
file = "/home/jbushart/tmp/wetter.com/wetter.com-9-0.dnstap.xz"
s1 = pylib.load_file(file)
s2 = pylib.load_file(file.replace("dnstap.xz", "pcap"))

print(s1)
print("-" * 100)
print(s2)
print("-" * 100)
pprint(s1.distance_with_details(s2))

# %%
print(f"{'Length':<15}:{s1.len():>10}{s2.len():>10}")
print(f"{'Message Count':<15}:{s1.message_count():>10}+4{s2.message_count():>10}+4")

# %% [markdown]
# # Load Wireshark Export
#
# Make sure the following configurations are correct before loading them here:
# * Only include DNS responses: `dns.flags.response == 1`
# * Export JSON using `Export Packet Dissections` → `As JSON...`


# %%
j = json.load(open("/home/jbushart/tmp/wetter.com/tmp.json"))

# %%
query_names = list()
for entry in j:
    queries = entry["_source"]["layers"]["dns"]["Queries"]
    assert len(queries) == 1
    query_name = next(iter(queries.values()))["dns.qry.name"]
    query_names.append(query_name)

print(
    f"In the pcap there are {len(query_names)} responses of which {len(set(query_names))} are unique."
)

# %% [markdown]
# ## Extract all frame numbers of real DNS responses

# %%
frame_numbers = [
    int(entry["_source"]["layers"]["frame"]["frame.number"]) for entry in j
]

# %% [markdown]
# ## Extract all frame numbers of the assumed DNS responses
#
# Generate the JSON like:
# ```
# extract_sequence  --verbose ~/tmp/realgm.com/*-3-0.pcap >/dev/null 2>~/tmp/realgm.com/filtered-dns.json
# ```

# %%
extracted_pcap = json.load(open("/home/jbushart/tmp/realgm.com/filtered-dns.json"))

# %%
assumed_frame_numbers = [entry["packet_in_pcap"] for entry in extracted_pcap]

# %%
len(frame_numbers), len(assumed_frame_numbers)

# %%
wrongly_assumed = sorted(list(set(assumed_frame_numbers) - set(frame_numbers)))
wrongly_assumed

# %%
missed_frames = sorted(list(set(frame_numbers[2:-2]) - set(assumed_frame_numbers)))
missed_frames

# %%
for entry in extracted_pcap:
    if entry["packet_in_pcap"] in wrongly_assumed:
        pprint(entry)

# %%
