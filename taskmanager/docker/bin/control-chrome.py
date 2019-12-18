#!/usr/bin/env python3
# pylint: disable=global-statement

import argparse
import json
import subprocess
import time
import typing as t
from subprocess import DEVNULL, STDOUT

from selenium import webdriver
from selenium.webdriver.common.desired_capabilities import DesiredCapabilities
from selenium.webdriver.firefox.firefox_profile import FirefoxProfile
from selenium.webdriver.firefox.options import Options

# Wait this many seconds after every browser event before a browser close can occur
WEBPAGE_TOTAL_TIME = 20.0

DNSTAP_SOCKET = "/var/run/unbound/dnstap.sock"
DNSTAP_FILE = "/output/website-log.dnstap"

PROC_STUBBY = None

PREFERENCES = {
    # Log console.log to stdout
    "devtools.console.stdout.content": True,
    "general.useragent.override": "Mozilla/5.0 (X11; Fedora; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/75.0.3770.100 Safari/537.36",
    # Disable Telemetry and background network
    "browser.safebrowsing.downloads.remote.enabled": False,
    "extensions.blocklist.enabled": False,
    "network.dns.disablePrefetch": True,
    "network.prefetch-next": False,
    "toolkit.telemetry.coverage.opt-out": True,
    "toolkit.telemetry.enabled": False,
    "toolkit.telemetry.unified": False,
    # Try to disable all captive portal detection stuff
    "network.captive-portal-service.enabled": False,
    "captivedetect.canonicalURL": "",
    "captivedetect.canonicalContent": "",
    # "captivedetect.maxRetryCount": 0,
    # "captivedetect.maxWaitingTime": 0,
    # "captivedetect.pollingTime": 0,
    # Make sure the browser never automatically loads a URL
    "browser.startup.homepage": "about:blank",
    "browser.newtabpage.pinned": "about:blank",
    "geo.enabled": False,
    "privacy.trackingprotection.pbmode.enabled": False,
    "network.connectivity-service.enabled": False,
}


def start_webdriver(profile_directory: t.Optional[str] = None) -> t.Any:
    d = DesiredCapabilities.FIREFOX
    d["loggingPrefs"] = {"browser": "ALL"}

    options = Options()
    options.headless = True
    options.add_argument("--safe-mode")

    profile = FirefoxProfile(profile_directory=profile_directory)

    for key, value in PREFERENCES.items():
        profile.set_preference(key, value)

    # Generate a preference file which can be placed into the global hierarchy
    pref_file = ""
    for key, value in PREFERENCES.items():
        pref_file += f"""pref({json.dumps(key)}, {json.dumps(value)});\n"""
    print(pref_file)
    subprocess.run(
        [
            "sudo",
            "tee",
            "-a",
            "/usr/lib64/firefox/browser/defaults/preferences/aa-dnscapture.js",
        ],
        input=pref_file.encode(),
        stdout=DEVNULL,
        check=True,
    )
    subprocess.run(
        [
            "sudo",
            "tee",
            "-a",
            "/usr/lib64/firefox/browser/defaults/preferences/zz-dnscapture.js",
        ],
        input=pref_file.encode(),
        stdout=DEVNULL,
        check=True,
    )

    driver = webdriver.Firefox(
        options=options,
        firefox_profile=profile,
        capabilities=d,
        log_path="/output/website-log.geckodriver.log",
    )
    driver.set_window_size(1920, 1080)
    driver.set_page_load_timeout(WEBPAGE_TOTAL_TIME)

    return driver


def handle_url(url: str) -> None:
    driver_tmp = start_webdriver()
    driver = start_webdriver(driver_tmp.profile.path)
    driver_tmp.close()
    del driver_tmp
    time.sleep(2)

    # Execute before experiment scripts
    before_experiment()
    driver.get(url)
    # Wait some time after the page load to make sure it is really loaded
    time.sleep(5)
    driver.save_screenshot("/output/website-log.screenshot.png")
    after_experiment()

    driver.close()


def before_experiment() -> None:
    print("Start before experiment")
    start_dns_software()
    print("Flush")
    subprocess.run(
        ["sudo", "unbound-control", "flush_zone", "."], stderr=STDOUT, check=True
    )
    subprocess.run(
        ["sudo", "unbound-control", "flush_bogus"], stderr=STDOUT, check=True
    )
    subprocess.run(
        ["sudo", "unbound-control", "flush_zone", "."], stderr=STDOUT, check=True
    )
    subprocess.run(
        ["sudo", "unbound-control", "flush_negative"], stderr=STDOUT, check=True
    )
    subprocess.run(
        ["sudo", "unbound-control", "flush_infra", "all"], stderr=STDOUT, check=True
    )

    print("Load cache file")
    with open("/output/cache.dump", "rb") as fin:
        subprocess.run(
            ["sudo", "unbound-control", "load_cache"],
            stdin=fin,
            stderr=STDOUT,
            check=True,
        )

    print("start.example marker query")
    subprocess.run(
        [
            "dig",
            "@127.0.0.1",
            "+tries=1",
            "A",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.",
        ],
        stdout=DEVNULL,
        stderr=DEVNULL,
        check=True,
    )
    subprocess.run(
        ["dig", "@127.0.0.1", "+tries=1", "A", "start.example."],
        stdout=DEVNULL,
        stderr=DEVNULL,
        check=True,
    )
    with open("/output/website-log.dnstimes.txt", "at") as fout:
        subprocess.run(["date", "+%s.%N"], stdout=fout, check=True)
    print("Finished before experiment")


def after_experiment() -> None:
    print("Start after experiment")

    print("end.example marker query")
    with open("/output/website-log.dnstimes.txt", "at") as fout:
        subprocess.run(["date", "+%s.%N"], stdout=fout, check=True)
    subprocess.run(
        ["dig", "@127.0.0.1", "+tries=1", "A", "end.example."],
        stdout=DEVNULL,
        stderr=DEVNULL,
        check=True,
    )
    subprocess.run(
        [
            "dig",
            "@127.0.0.1",
            "+tries=1",
            "A",
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz.zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz.zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz.zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz.",
        ],
        stdout=DEVNULL,
        stderr=DEVNULL,
        check=True,
    )
    print("Finished after experiment")


def start_dns_software() -> None:
    global PROC_STUBBY

    print("Starting Stubby and Unbound")
    PROC_STUBBY = subprocess.Popen(["stubby", "-g", "-C", "/etc/stubby/stubby.yml"])
    subprocess.run(["sudo", "unbound-control", "start"], stderr=STDOUT, check=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "url", metavar="URL", help="URL for which network dependencies should be loaded"
    )
    args = parser.parse_args()

    handle_url(args.url)


if __name__ == "__main__":
    main()
