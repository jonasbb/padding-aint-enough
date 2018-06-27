# Analyze Missclassifications

1. [Raw Data](#raw-data)
2. [Aggregated Data](#aggregated-data)
    1. [`360.cn`](#360cn)
    2. [`amazon.de`](#amazonde)
    3. [`bing.com`](#bingcom)
    4. [`coccoc.com`](#coccoccom)
    5. [`csdn.net`](#csdnnet)
    6. [`detail.tmall.com`](#detailtmallcom)
    7. [`espn.com`](#espncom)
    8. [`fbcdn.net`](#fbcdnnet)
    9. [`google.ca`](#googleca)

## Raw Data

Contains information about:

* the `k` of kNN
* the dnstap-file misclassified
* the expected label, might differ from the dnstap-file path in case of confusion domains
* the label returned by kNN

```text
K 1 Seq: ../dnscaptures_working/popads.net/website-log-1.dnstap, Expected: 'popads.net'  Got: 'Google Inc.'
K 1 Seq: ../dnscaptures_working/microsoftonline.com/website-log-1.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/reddit.com/website-log-1.dnstap, Expected: 'reddit.com'  Got: 'twitter.com'
K 1 Seq: ../dnscaptures_working/soso.com/website-log-1.dnstap, Expected: 'sogou.com'  Got: 'facebook.com'
K 1 Seq: ../dnscaptures_working/google.com.tw/website-log-1.dnstap, Expected: 'Google Inc.'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/360.cn/website-log-1.dnstap, Expected: '360.cn'  Got: 'paypal.com'
K 1 Seq: ../dnscaptures_working/csdn.net/website-log-1.dnstap, Expected: 'csdn.net'  Got: 'microsoft.com'
K 1 Seq: ../dnscaptures_working/coccoc.com/website-log-1.dnstap, Expected: 'coccoc.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/xhamster.com/website-log-1.dnstap, Expected: 'xhamster.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/popads.net/website-log-1.dnstap, Expected: 'popads.net'  Got: 'Google Inc.'
K 2 Seq: ../dnscaptures_working/microsoftonline.com/website-log-1.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/reddit.com/website-log-1.dnstap, Expected: 'reddit.com'  Got: 'twitter.com'
K 2 Seq: ../dnscaptures_working/soso.com/website-log-1.dnstap, Expected: 'sogou.com'  Got: 'facebook.com'
K 2 Seq: ../dnscaptures_working/pixnet.net/website-log-1.dnstap, Expected: 'pixnet.net'  Got: 'hao123.com - pixnet.net'
K 2 Seq: ../dnscaptures_working/google.com.tw/website-log-1.dnstap, Expected: 'Google Inc.'  Got: 't.co - whatsapp.com'
K 2 Seq: ../dnscaptures_working/360.cn/website-log-1.dnstap, Expected: '360.cn'  Got: 'paypal.com'
K 2 Seq: ../dnscaptures_working/csdn.net/website-log-1.dnstap, Expected: 'csdn.net'  Got: 'microsoft.com'
K 2 Seq: ../dnscaptures_working/coccoc.com/website-log-1.dnstap, Expected: 'coccoc.com'  Got: 't.co - whatsapp.com'
K 2 Seq: ../dnscaptures_working/xhamster.com/website-log-1.dnstap, Expected: 'xhamster.com'  Got: 't.co - whatsapp.com'
K 3 Seq: ../dnscaptures_working/popads.net/website-log-1.dnstap, Expected: 'popads.net'  Got: 'Google Inc.'
K 3 Seq: ../dnscaptures_working/microsoftonline.com/website-log-1.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/reddit.com/website-log-1.dnstap, Expected: 'reddit.com'  Got: 'twitter.com'
K 3 Seq: ../dnscaptures_working/soso.com/website-log-1.dnstap, Expected: 'sogou.com'  Got: 'facebook.com'
K 3 Seq: ../dnscaptures_working/google.com.tw/website-log-1.dnstap, Expected: 'Google Inc.'  Got: 'whatsapp.com'
K 3 Seq: ../dnscaptures_working/360.cn/website-log-1.dnstap, Expected: '360.cn'  Got: 'paypal.com'
K 3 Seq: ../dnscaptures_working/csdn.net/website-log-1.dnstap, Expected: 'csdn.net'  Got: 'microsoft.com'
K 3 Seq: ../dnscaptures_working/coccoc.com/website-log-1.dnstap, Expected: 'coccoc.com'  Got: 'whatsapp.com'
K 3 Seq: ../dnscaptures_working/xhamster.com/website-log-1.dnstap, Expected: 'xhamster.com'  Got: 'whatsapp.com'
K 4 Seq: ../dnscaptures_working/popads.net/website-log-1.dnstap, Expected: 'popads.net'  Got: 'Google Inc.'
K 4 Seq: ../dnscaptures_working/microsoftonline.com/website-log-1.dnstap, Expected: 'microsoftonline.com'  Got: 'microsoftonline.com - t.co'
K 4 Seq: ../dnscaptures_working/alipay.com/website-log-1.dnstap, Expected: 'alipay.com'  Got: 'alipay.com - so.com'
K 4 Seq: ../dnscaptures_working/reddit.com/website-log-1.dnstap, Expected: 'reddit.com'  Got: 'twitter.com'
K 4 Seq: ../dnscaptures_working/soso.com/website-log-1.dnstap, Expected: 'sogou.com'  Got: 'facebook.com'
K 4 Seq: ../dnscaptures_working/google.com.tw/website-log-1.dnstap, Expected: 'Google Inc.'  Got: 'whatsapp.com'
K 4 Seq: ../dnscaptures_working/t.co/website-log-1.dnstap, Expected: 't.co'  Got: 'microsoftonline.com - t.co'
K 4 Seq: ../dnscaptures_working/360.cn/website-log-1.dnstap, Expected: '360.cn'  Got: 'paypal.com'
K 4 Seq: ../dnscaptures_working/csdn.net/website-log-1.dnstap, Expected: 'csdn.net'  Got: 'microsoft.com'
K 4 Seq: ../dnscaptures_working/coccoc.com/website-log-1.dnstap, Expected: 'coccoc.com'  Got: 'whatsapp.com'
K 4 Seq: ../dnscaptures_working/xhamster.com/website-log-1.dnstap, Expected: 'xhamster.com'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/popads.net/website-log-1.dnstap, Expected: 'popads.net'  Got: 'Google Inc.'
K 5 Seq: ../dnscaptures_working/alipay.com/website-log-1.dnstap, Expected: 'alipay.com'  Got: 'so.com'
K 5 Seq: ../dnscaptures_working/reddit.com/website-log-1.dnstap, Expected: 'reddit.com'  Got: 'twitter.com'
K 5 Seq: ../dnscaptures_working/soso.com/website-log-1.dnstap, Expected: 'sogou.com'  Got: 'facebook.com'
K 5 Seq: ../dnscaptures_working/google.com.tw/website-log-1.dnstap, Expected: 'Google Inc.'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/t.co/website-log-1.dnstap, Expected: 't.co'  Got: 'microsoftonline.com'
K 5 Seq: ../dnscaptures_working/360.cn/website-log-1.dnstap, Expected: '360.cn'  Got: 'paypal.com'
K 5 Seq: ../dnscaptures_working/csdn.net/website-log-1.dnstap, Expected: 'csdn.net'  Got: 'microsoft.com'
K 5 Seq: ../dnscaptures_working/coccoc.com/website-log-1.dnstap, Expected: 'coccoc.com'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/xhamster.com/website-log-1.dnstap, Expected: 'xhamster.com'  Got: 'whatsapp.com'
K 1 Seq: ../dnscaptures_working/microsoftonline.com/website-log-10.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/soso.com/website-log-10.dnstap, Expected: 'sogou.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/t.co/website-log-10.dnstap, Expected: 't.co'  Got: 'netflix.com'
K 1 Seq: ../dnscaptures_working/csdn.net/website-log-10.dnstap, Expected: 'csdn.net'  Got: 'whatsapp.com'
K 1 Seq: ../dnscaptures_working/twitter.com/website-log-10.dnstap, Expected: 'twitter.com'  Got: 'bing.com'
K 1 Seq: ../dnscaptures_working/office.com/website-log-10.dnstap, Expected: 'office.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/netflix.com/website-log-10.dnstap, Expected: 'netflix.com'  Got: 'Google Inc. - netflix.com'
K 2 Seq: ../dnscaptures_working/microsoftonline.com/website-log-10.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/soso.com/website-log-10.dnstap, Expected: 'sogou.com'  Got: 't.co - whatsapp.com'
K 2 Seq: ../dnscaptures_working/t.co/website-log-10.dnstap, Expected: 't.co'  Got: 'netflix.com'
K 2 Seq: ../dnscaptures_working/google.co.jp/website-log-10.dnstap, Expected: 'Google Inc.'  Got: 'Google Inc. - instagram.com'
K 2 Seq: ../dnscaptures_working/csdn.net/website-log-10.dnstap, Expected: 'csdn.net'  Got: 'whatsapp.com'
K 2 Seq: ../dnscaptures_working/twitter.com/website-log-10.dnstap, Expected: 'twitter.com'  Got: 'bing.com - netflix.com'
K 2 Seq: ../dnscaptures_working/office.com/website-log-10.dnstap, Expected: 'office.com'  Got: 't.co - whatsapp.com'
K 3 Seq: ../dnscaptures_working/microsoftonline.com/website-log-10.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/soso.com/website-log-10.dnstap, Expected: 'sogou.com'  Got: 'Google Inc. - t.co - whatsapp.com'
K 3 Seq: ../dnscaptures_working/t.co/website-log-10.dnstap, Expected: 't.co'  Got: 'netflix.com'
K 3 Seq: ../dnscaptures_working/google.co.jp/website-log-10.dnstap, Expected: 'Google Inc.'  Got: 'Google Inc. - bing.com - instagram.com'
K 3 Seq: ../dnscaptures_working/csdn.net/website-log-10.dnstap, Expected: 'csdn.net'  Got: 'whatsapp.com'
K 3 Seq: ../dnscaptures_working/twitter.com/website-log-10.dnstap, Expected: 'twitter.com'  Got: 'bing.com - netflix.com - so.com'
K 3 Seq: ../dnscaptures_working/office.com/website-log-10.dnstap, Expected: 'office.com'  Got: 'Google Inc. - t.co - whatsapp.com'
K 4 Seq: ../dnscaptures_working/microsoftonline.com/website-log-10.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 4 Seq: ../dnscaptures_working/soso.com/website-log-10.dnstap, Expected: 'sogou.com'  Got: 'whatsapp.com'
K 4 Seq: ../dnscaptures_working/t.co/website-log-10.dnstap, Expected: 't.co'  Got: 'netflix.com'
K 4 Seq: ../dnscaptures_working/csdn.net/website-log-10.dnstap, Expected: 'csdn.net'  Got: 'tumblr.com - whatsapp.com'
K 4 Seq: ../dnscaptures_working/twitter.com/website-log-10.dnstap, Expected: 'twitter.com'  Got: 'bing.com - instagram.com - netflix.com - so.com'
K 4 Seq: ../dnscaptures_working/office.com/website-log-10.dnstap, Expected: 'office.com'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/microsoftonline.com/website-log-10.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 5 Seq: ../dnscaptures_working/soso.com/website-log-10.dnstap, Expected: 'sogou.com'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/t.co/website-log-10.dnstap, Expected: 't.co'  Got: 'netflix.com'
K 5 Seq: ../dnscaptures_working/csdn.net/website-log-10.dnstap, Expected: 'csdn.net'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/twitter.com/website-log-10.dnstap, Expected: 'twitter.com'  Got: 'bing.com - deloton.com - instagram.com - netflix.com - so.com'
K 5 Seq: ../dnscaptures_working/office.com/website-log-10.dnstap, Expected: 'office.com'  Got: 'whatsapp.com'
K 1 Seq: ../dnscaptures_working/google.it/website-log-2.dnstap, Expected: 'Google Inc.'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/microsoftonline.com/website-log-2.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/office.com/website-log-2.dnstap, Expected: 'office.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/google.it/website-log-2.dnstap, Expected: 'Google Inc.'  Got: 't.co - whatsapp.com'
K 2 Seq: ../dnscaptures_working/microsoftonline.com/website-log-2.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/soso.com/website-log-2.dnstap, Expected: 'sogou.com'  Got: 'Google Inc. - sogou.com'
K 2 Seq: ../dnscaptures_working/office.com/website-log-2.dnstap, Expected: 'office.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/google.it/website-log-2.dnstap, Expected: 'Google Inc.'  Got: 'Google Inc. - t.co - whatsapp.com'
K 3 Seq: ../dnscaptures_working/microsoftonline.com/website-log-2.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/soso.com/website-log-2.dnstap, Expected: 'sogou.com'  Got: 'Google Inc. - csdn.net - sogou.com'
K 3 Seq: ../dnscaptures_working/office.com/website-log-2.dnstap, Expected: 'office.com'  Got: 't.co'
K 4 Seq: ../dnscaptures_working/google.it/website-log-2.dnstap, Expected: 'Google Inc.'  Got: 'whatsapp.com'
K 4 Seq: ../dnscaptures_working/microsoftonline.com/website-log-2.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 4 Seq: ../dnscaptures_working/soso.com/website-log-2.dnstap, Expected: 'sogou.com'  Got: 'Google Inc.'
K 4 Seq: ../dnscaptures_working/t.co/website-log-2.dnstap, Expected: 't.co'  Got: 'microsoftonline.com - t.co'
K 4 Seq: ../dnscaptures_working/office.com/website-log-2.dnstap, Expected: 'office.com'  Got: 't.co'
K 5 Seq: ../dnscaptures_working/google.it/website-log-2.dnstap, Expected: 'Google Inc.'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/microsoftonline.com/website-log-2.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 5 Seq: ../dnscaptures_working/soso.com/website-log-2.dnstap, Expected: 'sogou.com'  Got: 'Google Inc. - sogou.com'
K 5 Seq: ../dnscaptures_working/t.co/website-log-2.dnstap, Expected: 't.co'  Got: 'microsoftonline.com'
K 5 Seq: ../dnscaptures_working/office.com/website-log-2.dnstap, Expected: 'office.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/microsoftonline.com/website-log-3.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/tmall.com/website-log-3.dnstap, Expected: 'tmall.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/twitch.tv/website-log-3.dnstap, Expected: 'twitch.tv'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/vk.com/website-log-3.dnstap, Expected: 'vk.com'  Got: 'blogger.com'
K 2 Seq: ../dnscaptures_working/microsoftonline.com/website-log-3.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/tmall.com/website-log-3.dnstap, Expected: 'tmall.com'  Got: 't.co - whatsapp.com'
K 2 Seq: ../dnscaptures_working/twitch.tv/website-log-3.dnstap, Expected: 'twitch.tv'  Got: 't.co - whatsapp.com'
K 2 Seq: ../dnscaptures_working/google.fr/website-log-3.dnstap, Expected: 'Google Inc.'  Got: 'Google Inc. - instagram.com'
K 2 Seq: ../dnscaptures_working/vk.com/website-log-3.dnstap, Expected: 'vk.com'  Got: 'blogger.com - vk.com'
K 3 Seq: ../dnscaptures_working/microsoftonline.com/website-log-3.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/tmall.com/website-log-3.dnstap, Expected: 'tmall.com'  Got: 'Google Inc. - t.co - whatsapp.com'
K 3 Seq: ../dnscaptures_working/twitch.tv/website-log-3.dnstap, Expected: 'twitch.tv'  Got: 'Google Inc. - t.co - whatsapp.com'
K 3 Seq: ../dnscaptures_working/google.fr/website-log-3.dnstap, Expected: 'Google Inc.'  Got: 'Google Inc. - bing.com - instagram.com'
K 4 Seq: ../dnscaptures_working/microsoftonline.com/website-log-3.dnstap, Expected: 'microsoftonline.com'  Got: 'microsoftonline.com - t.co'
K 4 Seq: ../dnscaptures_working/tmall.com/website-log-3.dnstap, Expected: 'tmall.com'  Got: 'whatsapp.com'
K 4 Seq: ../dnscaptures_working/t.co/website-log-3.dnstap, Expected: 't.co'  Got: 'microsoftonline.com - t.co'
K 4 Seq: ../dnscaptures_working/csdn.net/website-log-3.dnstap, Expected: 'csdn.net'  Got: 'csdn.net - sogou.com'
K 4 Seq: ../dnscaptures_working/twitch.tv/website-log-3.dnstap, Expected: 'twitch.tv'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/tmall.com/website-log-3.dnstap, Expected: 'tmall.com'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/t.co/website-log-3.dnstap, Expected: 't.co'  Got: 'microsoftonline.com'
K 5 Seq: ../dnscaptures_working/twitch.tv/website-log-3.dnstap, Expected: 'twitch.tv'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/pages.tmall.com/website-log-3.dnstap, Expected: 'pages.tmall.com'  Got: 'msn.com - pages.tmall.com'
K 1 Seq: ../dnscaptures_working/netflix.com/website-log-4.dnstap, Expected: 'netflix.com'  Got: 'wordpress.com'
K 1 Seq: ../dnscaptures_working/microsoftonline.com/website-log-4.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/netflix.com/website-log-4.dnstap, Expected: 'netflix.com'  Got: 'wordpress.com'
K 2 Seq: ../dnscaptures_working/microsoftonline.com/website-log-4.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/netflix.com/website-log-4.dnstap, Expected: 'netflix.com'  Got: 'wordpress.com'
K 3 Seq: ../dnscaptures_working/microsoftonline.com/website-log-4.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 4 Seq: ../dnscaptures_working/netflix.com/website-log-4.dnstap, Expected: 'netflix.com'  Got: 'wordpress.com'
K 4 Seq: ../dnscaptures_working/microsoftonline.com/website-log-4.dnstap, Expected: 'microsoftonline.com'  Got: 'microsoftonline.com - t.co'
K 4 Seq: ../dnscaptures_working/t.co/website-log-4.dnstap, Expected: 't.co'  Got: 'microsoftonline.com - t.co'
K 5 Seq: ../dnscaptures_working/netflix.com/website-log-4.dnstap, Expected: 'netflix.com'  Got: 'wordpress.com'
K 5 Seq: ../dnscaptures_working/t.co/website-log-4.dnstap, Expected: 't.co'  Got: 'microsoftonline.com'
K 1 Seq: ../dnscaptures_working/whatsapp.com/website-log-5.dnstap, Expected: 'whatsapp.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/microsoftonline.com/website-log-5.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/reddit.com/website-log-5.dnstap, Expected: 'reddit.com'  Got: 'wikia.com'
K 2 Seq: ../dnscaptures_working/whatsapp.com/website-log-5.dnstap, Expected: 'whatsapp.com'  Got: 't.co - whatsapp.com'
K 2 Seq: ../dnscaptures_working/microsoftonline.com/website-log-5.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/reddit.com/website-log-5.dnstap, Expected: 'reddit.com'  Got: 'wikia.com - youtube.com'
K 3 Seq: ../dnscaptures_working/whatsapp.com/website-log-5.dnstap, Expected: 'whatsapp.com'  Got: 'Google Inc. - t.co - whatsapp.com'
K 3 Seq: ../dnscaptures_working/microsoftonline.com/website-log-5.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/reddit.com/website-log-5.dnstap, Expected: 'reddit.com'  Got: 'wikia.com'
K 4 Seq: ../dnscaptures_working/whatsapp.com/website-log-5.dnstap, Expected: 'whatsapp.com'  Got: 'Google Inc. - t.co - tmall.com - whatsapp.com'
K 4 Seq: ../dnscaptures_working/microsoftonline.com/website-log-5.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 4 Seq: ../dnscaptures_working/reddit.com/website-log-5.dnstap, Expected: 'reddit.com'  Got: 'wikia.com - youtube.com'
K 4 Seq: ../dnscaptures_working/t.co/website-log-5.dnstap, Expected: 't.co'  Got: 'microsoftonline.com - t.co'
K 5 Seq: ../dnscaptures_working/whatsapp.com/website-log-5.dnstap, Expected: 'whatsapp.com'  Got: 'Google Inc. - sogou.com - t.co - tmall.com - whatsapp.com'
K 5 Seq: ../dnscaptures_working/microsoftonline.com/website-log-5.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 5 Seq: ../dnscaptures_working/reddit.com/website-log-5.dnstap, Expected: 'reddit.com'  Got: 'wikia.com - youtube.com'
K 5 Seq: ../dnscaptures_working/soso.com/website-log-5.dnstap, Expected: 'sogou.com'  Got: 'Google Inc. - sogou.com'
K 5 Seq: ../dnscaptures_working/t.co/website-log-5.dnstap, Expected: 't.co'  Got: 'microsoftonline.com'
K 5 Seq: ../dnscaptures_working/pages.tmall.com/website-log-5.dnstap, Expected: 'pages.tmall.com'  Got: 'pages.tmall.com - pinterest.com'
K 1 Seq: ../dnscaptures_working/amazon.de/website-log-6.dnstap, Expected: 'amazon.de'  Got: 'amazon.in'
K 1 Seq: ../dnscaptures_working/microsoftonline.com/website-log-6.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/soso.com/website-log-6.dnstap, Expected: 'sogou.com'  Got: 'whatsapp.com'
K 1 Seq: ../dnscaptures_working/t.co/website-log-6.dnstap, Expected: 't.co'  Got: 'whatsapp.com'
K 1 Seq: ../dnscaptures_working/linkedin.com/website-log-6.dnstap, Expected: 'linkedin.com'  Got: 'whatsapp.com'
K 1 Seq: ../dnscaptures_working/instagram.com/website-log-6.dnstap, Expected: 'instagram.com'  Got: 'Google Inc.'
K 1 Seq: ../dnscaptures_working/office.com/website-log-6.dnstap, Expected: 'office.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/fbcdn.net/website-log-6.dnstap, Expected: 'facebook.com'  Got: 'netflix.com'
K 2 Seq: ../dnscaptures_working/amazon.de/website-log-6.dnstap, Expected: 'amazon.de'  Got: 'amazon.de - amazon.in'
K 2 Seq: ../dnscaptures_working/microsoftonline.com/website-log-6.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/soso.com/website-log-6.dnstap, Expected: 'sogou.com'  Got: 'Google Inc. - whatsapp.com'
K 2 Seq: ../dnscaptures_working/t.co/website-log-6.dnstap, Expected: 't.co'  Got: 'Google Inc. - whatsapp.com'
K 2 Seq: ../dnscaptures_working/linkedin.com/website-log-6.dnstap, Expected: 'linkedin.com'  Got: 'Google Inc. - whatsapp.com'
K 2 Seq: ../dnscaptures_working/instagram.com/website-log-6.dnstap, Expected: 'instagram.com'  Got: 'Google Inc. - bing.com'
K 2 Seq: ../dnscaptures_working/office.com/website-log-6.dnstap, Expected: 'office.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/fbcdn.net/website-log-6.dnstap, Expected: 'facebook.com'  Got: 'netflix.com'
K 3 Seq: ../dnscaptures_working/microsoftonline.com/website-log-6.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/soso.com/website-log-6.dnstap, Expected: 'sogou.com'  Got: 'whatsapp.com'
K 3 Seq: ../dnscaptures_working/t.co/website-log-6.dnstap, Expected: 't.co'  Got: 'whatsapp.com'
K 3 Seq: ../dnscaptures_working/linkedin.com/website-log-6.dnstap, Expected: 'linkedin.com'  Got: 'whatsapp.com'
K 3 Seq: ../dnscaptures_working/instagram.com/website-log-6.dnstap, Expected: 'instagram.com'  Got: 'Google Inc. - bing.com - twitter.com'
K 3 Seq: ../dnscaptures_working/office.com/website-log-6.dnstap, Expected: 'office.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/fbcdn.net/website-log-6.dnstap, Expected: 'facebook.com'  Got: 'netflix.com'
K 4 Seq: ../dnscaptures_working/microsoftonline.com/website-log-6.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 4 Seq: ../dnscaptures_working/soso.com/website-log-6.dnstap, Expected: 'sogou.com'  Got: 'whatsapp.com'
K 4 Seq: ../dnscaptures_working/t.co/website-log-6.dnstap, Expected: 't.co'  Got: 'whatsapp.com'
K 4 Seq: ../dnscaptures_working/linkedin.com/website-log-6.dnstap, Expected: 'linkedin.com'  Got: 'whatsapp.com'
K 4 Seq: ../dnscaptures_working/instagram.com/website-log-6.dnstap, Expected: 'instagram.com'  Got: 'Google Inc. - alipay.com - bing.com - twitter.com'
K 4 Seq: ../dnscaptures_working/office.com/website-log-6.dnstap, Expected: 'office.com'  Got: 't.co'
K 4 Seq: ../dnscaptures_working/fbcdn.net/website-log-6.dnstap, Expected: 'facebook.com'  Got: 'netflix.com'
K 5 Seq: ../dnscaptures_working/microsoftonline.com/website-log-6.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 5 Seq: ../dnscaptures_working/soso.com/website-log-6.dnstap, Expected: 'sogou.com'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/t.co/website-log-6.dnstap, Expected: 't.co'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/linkedin.com/website-log-6.dnstap, Expected: 'linkedin.com'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/instagram.com/website-log-6.dnstap, Expected: 'instagram.com'  Got: 'Google Inc.'
K 5 Seq: ../dnscaptures_working/office.com/website-log-6.dnstap, Expected: 'office.com'  Got: 't.co'
K 5 Seq: ../dnscaptures_working/fbcdn.net/website-log-6.dnstap, Expected: 'facebook.com'  Got: 'netflix.com'
K 1 Seq: ../dnscaptures_working/amazon.de/website-log-7.dnstap, Expected: 'amazon.de'  Got: 'amazon.in'
K 1 Seq: ../dnscaptures_working/microsoftonline.com/website-log-7.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/google.ca/website-log-7.dnstap, Expected: 'Google Inc.'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/reddit.com/website-log-7.dnstap, Expected: 'reddit.com'  Got: 'taobao.com'
K 1 Seq: ../dnscaptures_working/xhamster.com/website-log-7.dnstap, Expected: 'xhamster.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/office.com/website-log-7.dnstap, Expected: 'office.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/mail.ru/website-log-7.dnstap, Expected: 'mail.ru'  Got: 'espn.com - mail.ru'
K 2 Seq: ../dnscaptures_working/amazon.de/website-log-7.dnstap, Expected: 'amazon.de'  Got: 'amazon.in'
K 2 Seq: ../dnscaptures_working/detail.tmall.com/website-log-7.dnstap, Expected: 'detail.tmall.com'  Got: 'Google Inc. - detail.tmall.com'
K 2 Seq: ../dnscaptures_working/microsoftonline.com/website-log-7.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/alipay.com/website-log-7.dnstap, Expected: 'alipay.com'  Got: 'alipay.com - github.com'
K 2 Seq: ../dnscaptures_working/google.ca/website-log-7.dnstap, Expected: 'Google Inc.'  Got: 't.co - whatsapp.com'
K 2 Seq: ../dnscaptures_working/reddit.com/website-log-7.dnstap, Expected: 'reddit.com'  Got: 'taobao.com'
K 2 Seq: ../dnscaptures_working/xhamster.com/website-log-7.dnstap, Expected: 'xhamster.com'  Got: 't.co - whatsapp.com'
K 2 Seq: ../dnscaptures_working/office.com/website-log-7.dnstap, Expected: 'office.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/amazon.de/website-log-7.dnstap, Expected: 'amazon.de'  Got: 'amazon.in'
K 3 Seq: ../dnscaptures_working/detail.tmall.com/website-log-7.dnstap, Expected: 'detail.tmall.com'  Got: 'Google Inc. - detail.tmall.com - netflix.com'
K 3 Seq: ../dnscaptures_working/microsoftonline.com/website-log-7.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/alipay.com/website-log-7.dnstap, Expected: 'alipay.com'  Got: 'github.com'
K 3 Seq: ../dnscaptures_working/google.ca/website-log-7.dnstap, Expected: 'Google Inc.'  Got: 'Google Inc. - t.co - whatsapp.com'
K 3 Seq: ../dnscaptures_working/reddit.com/website-log-7.dnstap, Expected: 'reddit.com'  Got: 'taobao.com'
K 3 Seq: ../dnscaptures_working/xhamster.com/website-log-7.dnstap, Expected: 'xhamster.com'  Got: 'Google Inc. - t.co - whatsapp.com'
K 3 Seq: ../dnscaptures_working/office.com/website-log-7.dnstap, Expected: 'office.com'  Got: 't.co'
K 4 Seq: ../dnscaptures_working/amazon.de/website-log-7.dnstap, Expected: 'amazon.de'  Got: 'amazon.in'
K 4 Seq: ../dnscaptures_working/detail.tmall.com/website-log-7.dnstap, Expected: 'detail.tmall.com'  Got: 'Google Inc.'
K 4 Seq: ../dnscaptures_working/microsoftonline.com/website-log-7.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 4 Seq: ../dnscaptures_working/alipay.com/website-log-7.dnstap, Expected: 'alipay.com'  Got: 'github.com'
K 4 Seq: ../dnscaptures_working/google.ca/website-log-7.dnstap, Expected: 'Google Inc.'  Got: 'whatsapp.com'
K 4 Seq: ../dnscaptures_working/reddit.com/website-log-7.dnstap, Expected: 'reddit.com'  Got: 'taobao.com'
K 4 Seq: ../dnscaptures_working/t.co/website-log-7.dnstap, Expected: 't.co'  Got: 'microsoftonline.com - t.co'
K 4 Seq: ../dnscaptures_working/xhamster.com/website-log-7.dnstap, Expected: 'xhamster.com'  Got: 'whatsapp.com'
K 4 Seq: ../dnscaptures_working/office.com/website-log-7.dnstap, Expected: 'office.com'  Got: 't.co'
K 5 Seq: ../dnscaptures_working/amazon.de/website-log-7.dnstap, Expected: 'amazon.de'  Got: 'amazon.in'
K 5 Seq: ../dnscaptures_working/detail.tmall.com/website-log-7.dnstap, Expected: 'detail.tmall.com'  Got: 'Google Inc.'
K 5 Seq: ../dnscaptures_working/microsoftonline.com/website-log-7.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 5 Seq: ../dnscaptures_working/alipay.com/website-log-7.dnstap, Expected: 'alipay.com'  Got: 'github.com'
K 5 Seq: ../dnscaptures_working/google.ca/website-log-7.dnstap, Expected: 'Google Inc.'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/reddit.com/website-log-7.dnstap, Expected: 'reddit.com'  Got: 'taobao.com'
K 5 Seq: ../dnscaptures_working/t.co/website-log-7.dnstap, Expected: 't.co'  Got: 'microsoftonline.com'
K 5 Seq: ../dnscaptures_working/xhamster.com/website-log-7.dnstap, Expected: 'xhamster.com'  Got: 'whatsapp.com'
K 5 Seq: ../dnscaptures_working/office.com/website-log-7.dnstap, Expected: 'office.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/whatsapp.com/website-log-8.dnstap, Expected: 'whatsapp.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/qq.com/website-log-8.dnstap, Expected: 'qq.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/detail.tmall.com/website-log-8.dnstap, Expected: 'detail.tmall.com'  Got: 'Google Inc.'
K 1 Seq: ../dnscaptures_working/microsoftonline.com/website-log-8.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/tumblr.com/website-log-8.dnstap, Expected: 'tumblr.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/soso.com/website-log-8.dnstap, Expected: 'sogou.com'  Got: 'csdn.net'
K 1 Seq: ../dnscaptures_working/bing.com/website-log-8.dnstap, Expected: 'bing.com'  Got: 'twitter.com'
K 2 Seq: ../dnscaptures_working/popads.net/website-log-8.dnstap, Expected: 'popads.net'  Got: 'netflix.com - popads.net'
K 2 Seq: ../dnscaptures_working/amazon.de/website-log-8.dnstap, Expected: 'amazon.de'  Got: 'amazon.de - amazon.in'
K 2 Seq: ../dnscaptures_working/whatsapp.com/website-log-8.dnstap, Expected: 'whatsapp.com'  Got: 'Google Inc. - t.co'
K 2 Seq: ../dnscaptures_working/qq.com/website-log-8.dnstap, Expected: 'qq.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/detail.tmall.com/website-log-8.dnstap, Expected: 'detail.tmall.com'  Got: 'Google Inc. - detail.tmall.com'
K 2 Seq: ../dnscaptures_working/microsoftonline.com/website-log-8.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/tumblr.com/website-log-8.dnstap, Expected: 'tumblr.com'  Got: 'Google Inc. - t.co'
K 2 Seq: ../dnscaptures_working/soso.com/website-log-8.dnstap, Expected: 'sogou.com'  Got: 'csdn.net - sogou.com'
K 2 Seq: ../dnscaptures_working/bing.com/website-log-8.dnstap, Expected: 'bing.com'  Got: 'Google Inc. - twitter.com'
K 3 Seq: ../dnscaptures_working/amazon.de/website-log-8.dnstap, Expected: 'amazon.de'  Got: 'amazon.in'
K 3 Seq: ../dnscaptures_working/whatsapp.com/website-log-8.dnstap, Expected: 'whatsapp.com'  Got: 'Google Inc. - t.co - whatsapp.com'
K 3 Seq: ../dnscaptures_working/qq.com/website-log-8.dnstap, Expected: 'qq.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/detail.tmall.com/website-log-8.dnstap, Expected: 'detail.tmall.com'  Got: 'Google Inc. - detail.tmall.com - sogou.com'
K 3 Seq: ../dnscaptures_working/microsoftonline.com/website-log-8.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/tumblr.com/website-log-8.dnstap, Expected: 'tumblr.com'  Got: 'Google Inc. - t.co - whatsapp.com'
K 3 Seq: ../dnscaptures_working/soso.com/website-log-8.dnstap, Expected: 'sogou.com'  Got: 'csdn.net'
K 3 Seq: ../dnscaptures_working/bing.com/website-log-8.dnstap, Expected: 'bing.com'  Got: 'Google Inc. - instagram.com - twitter.com'
K 4 Seq: ../dnscaptures_working/amazon.de/website-log-8.dnstap, Expected: 'amazon.de'  Got: 'amazon.de - amazon.in'
K 4 Seq: ../dnscaptures_working/whatsapp.com/website-log-8.dnstap, Expected: 'whatsapp.com'  Got: 'Google Inc. - t.co - tmall.com - whatsapp.com'
K 4 Seq: ../dnscaptures_working/qq.com/website-log-8.dnstap, Expected: 'qq.com'  Got: 'microsoftonline.com - t.co'
K 4 Seq: ../dnscaptures_working/detail.tmall.com/website-log-8.dnstap, Expected: 'detail.tmall.com'  Got: 'Google Inc. - detail.tmall.com - netflix.com - sogou.com'
K 4 Seq: ../dnscaptures_working/microsoftonline.com/website-log-8.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 4 Seq: ../dnscaptures_working/tumblr.com/website-log-8.dnstap, Expected: 'tumblr.com'  Got: 'Google Inc. - t.co - tmall.com - whatsapp.com'
K 4 Seq: ../dnscaptures_working/soso.com/website-log-8.dnstap, Expected: 'sogou.com'  Got: 'csdn.net - sogou.com'
K 4 Seq: ../dnscaptures_working/bing.com/website-log-8.dnstap, Expected: 'bing.com'  Got: 'Google Inc. - instagram.com - netflix.com - twitter.com'
K 5 Seq: ../dnscaptures_working/amazon.de/website-log-8.dnstap, Expected: 'amazon.de'  Got: 'amazon.in'
K 5 Seq: ../dnscaptures_working/whatsapp.com/website-log-8.dnstap, Expected: 'whatsapp.com'  Got: 'Google Inc. - sogou.com - t.co - tmall.com - whatsapp.com'
K 5 Seq: ../dnscaptures_working/qq.com/website-log-8.dnstap, Expected: 'qq.com'  Got: 'microsoftonline.com'
K 5 Seq: ../dnscaptures_working/detail.tmall.com/website-log-8.dnstap, Expected: 'detail.tmall.com'  Got: 'Google Inc.'
K 5 Seq: ../dnscaptures_working/microsoftonline.com/website-log-8.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 5 Seq: ../dnscaptures_working/tumblr.com/website-log-8.dnstap, Expected: 'tumblr.com'  Got: 'Google Inc. - sogou.com - t.co - tmall.com - whatsapp.com'
K 5 Seq: ../dnscaptures_working/bing.com/website-log-8.dnstap, Expected: 'bing.com'  Got: 'Google Inc. - alipay.com - instagram.com - netflix.com - twitter.com'
K 1 Seq: ../dnscaptures_working/espn.com/website-log-9.dnstap, Expected: 'espn.com'  Got: 'ebay.com'
K 1 Seq: ../dnscaptures_working/qq.com/website-log-9.dnstap, Expected: 'qq.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/microsoftonline.com/website-log-9.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 1 Seq: ../dnscaptures_working/reddit.com/website-log-9.dnstap, Expected: 'reddit.com'  Got: 'netflix.com'
K 1 Seq: ../dnscaptures_working/fbcdn.net/website-log-9.dnstap, Expected: 'facebook.com'  Got: 'tumblr.com'
K 2 Seq: ../dnscaptures_working/espn.com/website-log-9.dnstap, Expected: 'espn.com'  Got: 'ebay.com'
K 2 Seq: ../dnscaptures_working/qq.com/website-log-9.dnstap, Expected: 'qq.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/microsoftonline.com/website-log-9.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 2 Seq: ../dnscaptures_working/reddit.com/website-log-9.dnstap, Expected: 'reddit.com'  Got: 'netflix.com'
K 2 Seq: ../dnscaptures_working/fbcdn.net/website-log-9.dnstap, Expected: 'facebook.com'  Got: 'tumblr.com'
K 2 Seq: ../dnscaptures_working/weibo.com/website-log-9.dnstap, Expected: 'weibo.com'  Got: 'taobao.com - weibo.com'
K 3 Seq: ../dnscaptures_working/espn.com/website-log-9.dnstap, Expected: 'espn.com'  Got: 'ebay.com'
K 3 Seq: ../dnscaptures_working/qq.com/website-log-9.dnstap, Expected: 'qq.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/microsoftonline.com/website-log-9.dnstap, Expected: 'microsoftonline.com'  Got: 't.co'
K 3 Seq: ../dnscaptures_working/reddit.com/website-log-9.dnstap, Expected: 'reddit.com'  Got: 'netflix.com'
K 3 Seq: ../dnscaptures_working/fbcdn.net/website-log-9.dnstap, Expected: 'facebook.com'  Got: 'tumblr.com'
K 3 Seq: ../dnscaptures_working/weibo.com/website-log-9.dnstap, Expected: 'weibo.com'  Got: 'taobao.com'
K 4 Seq: ../dnscaptures_working/espn.com/website-log-9.dnstap, Expected: 'espn.com'  Got: 'ebay.com'
K 4 Seq: ../dnscaptures_working/qq.com/website-log-9.dnstap, Expected: 'qq.com'  Got: 'microsoftonline.com - t.co'
K 4 Seq: ../dnscaptures_working/microsoftonline.com/website-log-9.dnstap, Expected: 'microsoftonline.com'  Got: 'microsoftonline.com - t.co'
K 4 Seq: ../dnscaptures_working/reddit.com/website-log-9.dnstap, Expected: 'reddit.com'  Got: 'netflix.com'
K 4 Seq: ../dnscaptures_working/fbcdn.net/website-log-9.dnstap, Expected: 'facebook.com'  Got: 'tumblr.com'
K 4 Seq: ../dnscaptures_working/weibo.com/website-log-9.dnstap, Expected: 'weibo.com'  Got: 'taobao.com'
K 5 Seq: ../dnscaptures_working/espn.com/website-log-9.dnstap, Expected: 'espn.com'  Got: 'ebay.com'
K 5 Seq: ../dnscaptures_working/qq.com/website-log-9.dnstap, Expected: 'qq.com'  Got: 'microsoftonline.com'
K 5 Seq: ../dnscaptures_working/reddit.com/website-log-9.dnstap, Expected: 'reddit.com'  Got: 'netflix.com'
K 5 Seq: ../dnscaptures_working/pages.tmall.com/website-log-9.dnstap, Expected: 'pages.tmall.com'  Got: 'pages.tmall.com - tmall.com'
K 5 Seq: ../dnscaptures_working/fbcdn.net/website-log-9.dnstap, Expected: 'facebook.com'  Got: 'tumblr.com'
K 5 Seq: ../dnscaptures_working/weibo.com/website-log-9.dnstap, Expected: 'weibo.com'  Got: 'taobao.com'
```

## Aggregated Data

Counts how often each file is being misclassified.
The most are 5 as the test was run for `k = 1..=5`.

```text
5 ../dnscaptures_working/360.cn/website-log-1.dnstap ✔
2 ../dnscaptures_working/alipay.com/website-log-1.p
4 ../dnscaptures_working/alipay.com/website-log-7.dnstap
2 ../dnscaptures_working/amazon.de/website-log-6.dnstap ✔
5 ../dnscaptures_working/amazon.de/website-log-7.dnstap ✔
4 ../dnscaptures_working/amazon.de/website-log-8.dnstap ✔
5 ../dnscaptures_working/bing.com/website-log-8.dnstap ✔
5 ../dnscaptures_working/coccoc.com/website-log-1.dnstap ✔
5 ../dnscaptures_working/csdn.net/website-log-10.dnstap ✔
5 ../dnscaptures_working/csdn.net/website-log-1.dnstap ✔
1 ../dnscaptures_working/csdn.net/website-log-3.dnstap ✔
4 ../dnscaptures_working/detail.tmall.com/website-log-7.dnstap ✔
5 ../dnscaptures_working/detail.tmall.com/website-log-8.dnstap ✔
5 ../dnscaptures_working/espn.com/website-log-9.dnstap ✔
5 ../dnscaptures_working/fbcdn.net/website-log-6.dnstap ✔
5 ../dnscaptures_working/fbcdn.net/website-log-9.dnstap ✔
5 ../dnscaptures_working/google.ca/website-log-7.dnstap ✔
2 ../dnscaptures_working/google.co.jp/website-log-10.dnstap
5 ../dnscaptures_working/google.com.tw/website-log-1.dnstap
2 ../dnscaptures_working/google.fr/website-log-3.dnstap
5 ../dnscaptures_working/google.it/website-log-2.dnstap
5 ../dnscaptures_working/instagram.com/website-log-6.dnstap
5 ../dnscaptures_working/linkedin.com/website-log-6.dnstap
1 ../dnscaptures_working/mail.ru/website-log-7.dnstap
5 ../dnscaptures_working/microsoftonline.com/website-log-10.dnstap
4 ../dnscaptures_working/microsoftonline.com/website-log-1.dnstap
5 ../dnscaptures_working/microsoftonline.com/website-log-2.dnstap
4 ../dnscaptures_working/microsoftonline.com/website-log-3.dnstap
4 ../dnscaptures_working/microsoftonline.com/website-log-4.dnstap
5 ../dnscaptures_working/microsoftonline.com/website-log-5.dnstap
5 ../dnscaptures_working/microsoftonline.com/website-log-6.dnstap
5 ../dnscaptures_working/microsoftonline.com/website-log-7.dnstap
5 ../dnscaptures_working/microsoftonline.com/website-log-8.dnstap
4 ../dnscaptures_working/microsoftonline.com/website-log-9.dnstap
1 ../dnscaptures_working/netflix.com/website-log-10.dnstap
5 ../dnscaptures_working/netflix.com/website-log-4.dnstap
5 ../dnscaptures_working/office.com/website-log-10.dnstap
5 ../dnscaptures_working/office.com/website-log-2.dnstap
5 ../dnscaptures_working/office.com/website-log-6.dnstap
5 ../dnscaptures_working/office.com/website-log-7.dnstap
1 ../dnscaptures_working/pages.tmall.com/website-log-3.dnstap
1 ../dnscaptures_working/pages.tmall.com/website-log-5.dnstap
1 ../dnscaptures_working/pages.tmall.com/website-log-9.dnstap
1 ../dnscaptures_working/pixnet.net/website-log-1.dnstap
5 ../dnscaptures_working/popads.net/website-log-1.dnstap
1 ../dnscaptures_working/popads.net/website-log-8.dnstap
5 ../dnscaptures_working/qq.com/website-log-8.dnstap
5 ../dnscaptures_working/qq.com/website-log-9.dnstap
5 ../dnscaptures_working/reddit.com/website-log-1.dnstap
5 ../dnscaptures_working/reddit.com/website-log-5.dnstap
5 ../dnscaptures_working/reddit.com/website-log-7.dnstap
5 ../dnscaptures_working/reddit.com/website-log-9.dnstap
5 ../dnscaptures_working/soso.com/website-log-10.dnstap
5 ../dnscaptures_working/soso.com/website-log-1.dnstap
4 ../dnscaptures_working/soso.com/website-log-2.dnstap
1 ../dnscaptures_working/soso.com/website-log-5.dnstap
5 ../dnscaptures_working/soso.com/website-log-6.dnstap
4 ../dnscaptures_working/soso.com/website-log-8.dnstap
5 ../dnscaptures_working/t.co/website-log-10.dnstap
2 ../dnscaptures_working/t.co/website-log-1.dnstap
2 ../dnscaptures_working/t.co/website-log-2.dnstap
2 ../dnscaptures_working/t.co/website-log-3.dnstap
2 ../dnscaptures_working/t.co/website-log-4.dnstap
2 ../dnscaptures_working/t.co/website-log-5.dnstap
5 ../dnscaptures_working/t.co/website-log-6.dnstap
2 ../dnscaptures_working/t.co/website-log-7.dnstap
5 ../dnscaptures_working/tmall.com/website-log-3.dnstap
5 ../dnscaptures_working/tumblr.com/website-log-8.dnstap
5 ../dnscaptures_working/twitch.tv/website-log-3.dnstap
5 ../dnscaptures_working/twitter.com/website-log-10.dnstap
2 ../dnscaptures_working/vk.com/website-log-3.dnstap
4 ../dnscaptures_working/weibo.com/website-log-9.dnstap
5 ../dnscaptures_working/whatsapp.com/website-log-5.dnstap
5 ../dnscaptures_working/whatsapp.com/website-log-8.dnstap
5 ../dnscaptures_working/xhamster.com/website-log-1.dnstap
5 ../dnscaptures_working/xhamster.com/website-log-7.dnstap
```

### `360.cn`

* `paypal.com`

### `amazon.de`

* `amazon.de`
* `amazon.in`

### `bing.com`

Seems to be very easily confused.

1. `twitter.com`
2. `Google Inc. - twitter.com`
3. `Google Inc. - instagram.com - twitter.com`
4. `Google Inc. - instagram.com - netflix.com - twitter.com`
5. `Google Inc. - alipay.com - instagram.com - netflix.com - twitter.com`

### `coccoc.com`

1. `t.co`
2. `t.co - whatsapp.com`
3. `whatsapp.com`
4. `whatsapp.com`
5. `whatsapp.com`

### `csdn.net`

* Log 1
    * `microsoft.com`
* Log 3
    * `k=4` `csdn.net - sogou.com`
* Log 10
    1. `whatsapp.com`
    2. `whatsapp.com`
    3. `whatsapp.com`
    4. `tumblr.com - whatsapp.com`
    5. `whatsapp.com`

### `detail.tmall.com`

* Log 7
    1. n/a
    2. `Google Inc. - detail.tmall.com`
    3. `Google Inc. - detail.tmall.com - netflix.com`
    4. `Google Inc.`
    5. `Google Inc.`
* Log 8
    1. `Google Inc.`
    2. `Google Inc. - detail.tmall.com`
    3. `Google Inc. - detail.tmall.com - sogou.com`
    4. `Google Inc. - detail.tmall.com - netflix.com - sogou.com`
    5. `Google Inc.`

### `espn.com`

* `ebay.com`

### `fbcdn.net`

* Log 6
    * `netflix.com`
* Log 9
    * `tumblr.com`

### `google.ca`

1. `t.co`
2. `t.co - whatsapp.com`
3. `Google Inc. - t.co - whatsapp.com`
4. `whatsapp.com`
5. `whatsapp.com`
