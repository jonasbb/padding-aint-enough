# README

Mounting a shared folder between KVM and host.

Setting SELinux permissions:

```bash
sudo semanage fcontext -a -t svirt_image_t (pwd)"/share(/.*)?"
sudo restorecon -vR "./share/"
```

Mounting in guest:

```bash
sudo mount -t 9p -o trans=virtio rpmbuild /mnt
```

Get a list of effective TLDs used within the Alexa top X:

```bash
xsv select 2 alexa-top1m.20180611T0204.csv | head -30000 | xargs tldextract | cut -d ' ' -f 3 | sort -u >tlds
```

Split the Alexa list into multiple chunks.
Use the top 10k and distribute it into two chunkgs.

```bash
head -10000 alexa-top1m.20180611T0204.csv | xsv select --no-headers 2- | split --additional-suffix=.txt --number=r/2 --numeric-suffixes - alexa-top10000-rr
```

Run the test in the VMs:

```bash
stdbuf -oL -eL /mnt/scripts/foreach-domain.fish ./alexa-top10000-rr00.txt 2>&1 | ts | tee log.txt
```

Convert the output of `dns-sequences` into a CSV file with all the wrong classifications:

```bash
rg 'Seq:' "$INPUT" | xsv select --no-headers --delimiter=' ' 2,4,6,8 > "$OUTPUT"
```

Domains most often wrongly classified, based on the CSV above:

```bash
xsv select 3 "$INPUT" | sort | uniq -c | sort -n
```

Most common wrong classification results, based on the CSV above:

```bash
xsv select 3,4 "$INPUT" | sort | uniq -c | sort -n
```
