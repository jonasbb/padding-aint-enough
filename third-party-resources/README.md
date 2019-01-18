# README

Convert the `counts_per_domain.json` into a CSV file showing the most common third-party domains, counted by how many domains include it:

```sh
jq -r "[[.[][0]], keys_unsorted] | transpose | .[] | @csv" counts_per_domain.json | sort -rn
```
