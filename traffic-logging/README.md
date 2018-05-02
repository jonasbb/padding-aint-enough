# Traffic Logging

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
