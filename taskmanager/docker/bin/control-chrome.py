#!/usr/bin/env python3
import argparse
import json
import os.path
import signal
import subprocess
import time
import typing as t

# import IPython
import requests
from websocket import (
    WebSocketConnectionClosedException,
    WebSocketTimeoutException,
    create_connection as create_ws_connection,
)

# Wait this many seconds after every browser event before a browser close can occur
WAIT_SECONDS = 7
WEBPAGE_TOTAL_TIME = 30.0


def handle_url(url: str, special_url: str, chrome_debug_port: int) -> None:
    wsurl = get_wsurl_for_url(special_url, chrome_debug_port)
    ws = create_ws_connection(wsurl, timeout=2)
    ws.settimeout(WAIT_SECONDS + 1)
    # Enable Network module
    ws.send(json.dumps({"id": 0, "method": "Debugger.enable"}))
    ws.send(
        json.dumps(
            {
                "id": 10,
                "method": "Debugger.setAsyncCallStackDepth",
                "params": {"maxDepth": 64},
            }
        )
    )
    ws.send(json.dumps({"id": 20, "method": "Network.enable"}))
    ws.send(
        json.dumps(
            {
                "id": 30,
                "method": "Network.setCacheDisabled",
                "params": {"cacheDisabled": True},
            }
        )
    )
    ws.send(
        json.dumps(
            {
                "id": 32,
                "method": "Target.setAutoAttach",
                "params": {"autoAttach": True, "waitForDebuggerOnStart": True},
            }
        )
    )
    ws.send(
        json.dumps(
            {
                "id": 34,
                "method": "Target.setDiscoverTargets",
                "params": {"discover": True},
            }
        )
    )
    time.sleep(1)

    # Execute before experiment scripts
    print("Start 'before-experiment.fish'")
    subprocess.run("/usr/bin/before-experiment.fish", stdin=subprocess.DEVNULL)
    print("Finished 'before-experiment.fish'")

    # Go to target url
    ws.send(
        json.dumps(
            {
                "id": 40,
                "method": "Page.navigate",
                "params": {"url": url, "transitionType": "typed"},
            }
        )
    )

    # close browser if SIGALRM is received
    def close_browser_timeout(signum: int, _frame: t.Any) -> None:
        if signum == signal.SIGALRM:
            print("Close Chrome due to extended inactivity")
            ws.send(json.dumps({"id": 1000, "method": "Browser.close"}))

    signal.signal(signal.SIGALRM, close_browser_timeout)

    start = time.monotonic()
    msglist: t.List[t.Any] = list()
    try:
        for msg in ws:
            if time.monotonic() - start > WEBPAGE_TOTAL_TIME:
                print("Close Chrome due to total wall time limit")
                break
            signal.alarm(WAIT_SECONDS)
            data = json.loads(msg)
            if "id" in data:
                continue
            msglist.append(data)
        else:
            print("No more websocket messages from Chrome")
    except WebSocketTimeoutException:
        print("WEBSOCKET TIMEOUT EXCEPTION")
    except WebSocketConnectionClosedException:
        pass
    finally:
        json.dump(msglist, open("website-log.json", "w"))


def get_wsurl_for_url(url: str, port: int) -> str:
    """
    Return the corresponding websocket URL for the tab which currently has loaded the URL `url`.

    Raises an exception if the URL is not found.
    """
    # load debugger manifest file
    res = requests.get(f"http://localhost:{port}/json")
    res.raise_for_status()
    manifest = res.json()
    for elem in manifest:
        if elem["url"] == url:
            return elem["webSocketDebuggerUrl"]
    # nothing found
    raise Exception(
        f"The Chrome debugger does not have a tab instance for the URL '{url}'"
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "marker_url",
        metavar="marker",
        help="Marker URL used internally. Chrome must have this URL open",
    )
    parser.add_argument(
        "port", metavar="PORT", help="Port for Chrome's remote debugging"
    )
    parser.add_argument(
        "url", metavar="URL", help="URL for which network dependencies should be loaded"
    )
    args = parser.parse_args()

    handle_url(args.url, args.marker_url, args.port)


if __name__ == "__main__":
    main()
