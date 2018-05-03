# Traffic Logging

1. [Debugger Protocol](#debugger-protocol)
    1. [Enable Debugger Module](#enable-debugger-module)
    2. [Force a Page Reload](#force-a-page-reload)
    3. [Navigate to Page](#navigate-to-page)

Chrome can be use with a remote debugger.
Documentation on the remote debugging can be found [here](https://chromedevtools.github.io/devtools-protocol/#remote).

```bash
# Start debuggee
google-chrome --remote-debugging-port=9222
# Start debugger
google-chrome --user-data-dir=./chrome-data-dir/
```

`localhost:9222` will then show a list of pages for which the developer tools can be opened.
`localhost:9222/json` shows the same in JSON.

Sending

```json
{"method": "Network.enable", "id": 1}
```

to the websocket will enable traffic logging.
Responses with method `Network.requestWillBeSent` will contain a `initiator`, which will be the JS location which is reponsible for manipulation the DOM.
See [`websocket-log.json`][./minimal-webpage/websocket-log.json] for details.

This should allow to extract constraints in the form of: This URL is only loaded if those scripts are already loaded.

## Unimplemented Features

### How does DNS prefetching work with network logs

<https://www.chromium.org/developers/design-documents/dns-prefetching>  
<https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/X-DNS-Prefetch-Control>

```html
<link rel="dns-prefetch" href="//host_name_to_prefetch.com">

```

## Debugger Protocol

[Protocol documentation](https://chromedevtools.github.io/devtools-protocol/).

### [Enable Debugger Module](https://chromedevtools.github.io/devtools-protocol/tot/Network#method-enable)

```json
{"id": 1, "method": "Network.enable", "params": {"maxPostDataSize": 65536}}
```

There are many `*.enable` methods for all different kinds of debugger modules.
The `params` is optional.

### [Force a Page Reload](https://chromedevtools.github.io/devtools-protocol/tot/Page#method-reload)

```json
{"id": 1, "method": "Page.reload", "params": {"ignoreCache": false}}
```

### [Navigate to Page](https://chromedevtools.github.io/devtools-protocol/tot/Page#method-navigate)

```json
{"id": 1, "method": "Page.navigate", "params": {"url": "https://google.com"}}
```
