#!/usr/bin/env python3
import argparse
import json
import os.path
import signal
import subprocess
import tempfile
import time
import typing as t

import IPython
import requests
from websocket import create_connection as create_ws_connection
from websocket import WebSocketTimeoutException

# Wait this many seconds after every browser event before a browser close can occur
WAIT_SECONDS = 5
CHROME_DEBUG_PORT = 9229


def handle_url(url: str) -> None:
    special_url = "file:///"

    with tempfile.TemporaryDirectory() as tmpdir:
        # create an empty "First Run" file to prevent chrome from showing the frist run dialog
        with open(os.path.join(tmpdir, "First Run"), "w") as f:
            f.write("")

        # spawn a new independend chrome instance
        with subprocess.Popen(
            [
                "google-chrome",
                # "--headless",
                f"--user-data-dir={tmpdir}",
                f"--remote-debugging-port={CHROME_DEBUG_PORT}",
                special_url
            ],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
        ) as chrome:
            # give chrome some time to fully start
            time.sleep(2)

            wsurl = get_wsurl_for_url(special_url)
            ws = create_ws_connection(wsurl)
            ws.settimeout(2)
            # Enable Network module
            ws.send(json.dumps({
                "id": 0,
                "method": "Debugger.enable",
            }))
            ws.send(
                json.dumps({
                    "id": 10,
                    "method": "Debugger.setAsyncCallStackDepth",
                    "params": {
                        "maxDepth": 64,
                    }
                }))
            ws.send(json.dumps({
                "id": 20,
                "method": "Network.enable",
            }))
            ws.send(
                json.dumps({
                    "id": 30,
                    "method": "Network.setCacheDisabled",
                    "params": {
                        "cacheDisabled": True,
                    }
                }))
            # Go to target url
            ws.send(
                json.dumps({
                    "id": 40,
                    "method": "Page.navigate",
                    "params": {
                        "url": url,
                    },
                }))

            # close browser if SIGALRM is received
            def close_browser_timeout(signum: int, _frame: t.Any) -> None:
                if signum == signal.SIGALRM:
                    ws.send(
                        json.dumps({
                            "id": 1000,
                            "method": "Browser.close",
                        }))

            signal.signal(signal.SIGALRM, close_browser_timeout)

            # TODO set an overall time limit, not just the timer
            msglist: t.List[str] = list()
            try:
                for msg in ws:
                    # print(msg)
                    signal.alarm(WAIT_SECONDS)
                    if "id" in msg:
                        continue
                    msglist.append(msg + "\n")
            except WebSocketTimeoutException:
                pass
            finally:
                with open("website-log.json", "w") as f:
                    f.writelines(msglist)

            # give some time for a clean exit
            time.sleep(3)
            chrome.kill()


def get_wsurl_for_url(url: str) -> str:
    """
    Return the corresponding websocket URL for the tab which currently has loaded the URL `url`.

    Raises an exception if the URL is not found.
    """
    # load debugger manifest file
    res = requests.get(f"http://localhost:{CHROME_DEBUG_PORT}/json")
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
        'urls',
        metavar="URL",
        nargs="+",
        help="URL for which network dependencies should be loaded")
    args = parser.parse_args()

    for url in args.urls:
        handle_url(url)


if __name__ == '__main__':
    main()
