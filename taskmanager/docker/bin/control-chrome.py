#!/usr/bin/env python3
# pylint: disable=global-statement

import argparse
import os
import subprocess
import time
import typing as t
from subprocess import DEVNULL, STDOUT

import tbselenium.common as cm
from selenium import webdriver
from selenium.webdriver.common.desired_capabilities import DesiredCapabilities
from selenium.webdriver.firefox.firefox_profile import FirefoxProfile
from selenium.webdriver.firefox.options import Options
from tbselenium.tbdriver import TorBrowserDriver
from tbselenium.utils import launch_tbb_tor_with_stem

# Configuration for the Tor Browser Bundle
TBB_DIR = "/opt/tor-browser_en-US"

# Wait this many seconds after every browser event before a browser close can occur
WEBPAGE_TOTAL_TIME = 20.0

DNSTAP_SOCKET = "/var/run/unbound/dnstap.sock"
DNSTAP_FILE = "/output/website-log.dnstap"

PROC_STUBBY = None
PROC_TOR_PROCESS = None

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
    global PROC_TOR_PROCESS

    d = DesiredCapabilities.FIREFOX
    d["loggingPrefs"] = {"browser": "ALL"}

    options = Options()
    options.headless = True
    options.add_argument("--safe-mode")

    profile = FirefoxProfile(profile_directory=profile_directory)

    for key, value in PREFERENCES.items():
        profile.set_preference(key, value)

    if os.getenv("USE_TOR", None) is not None:
        if PROC_TOR_PROCESS:
            PROC_TOR_PROCESS.kill()
        PROC_TOR_PROCESS = launch_tbb_tor_with_stem(tbb_path=TBB_DIR)
        driver = TorBrowserDriver(
            TBB_DIR,
            tor_cfg=cm.USE_STEM,
            options=options,
            tbb_profile_path=profile,
            capabilities=d,
            tbb_logfile_path="/output/website-log.geckodriver.log",
        )
    else:
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
    # Monkey patch the tbselenium dependency
    TorBrowserDriver.__init__ = new_init

    parser = argparse.ArgumentParser()
    parser.add_argument(
        "url", metavar="URL", help="URL for which network dependencies should be loaded"
    )
    args = parser.parse_args()

    handle_url(args.url)


# This is needed to add the options and capabilities to the constructor such that they can be passed
# down to the real Firefox webdriver.
#
# pylint: disable=too-many-arguments
def new_init(
    self: t.Any,
    tbb_path: str = "",
    tor_cfg: int = cm.USE_RUNNING_TOR,
    tbb_fx_binary_path: str = "",
    tbb_profile_path: str = "",
    tbb_logfile_path: str = "",
    tor_data_dir: str = "",
    pref_dict: t.Optional[t.Dict[str, t.Any]] = None,
    socks_port: t.Optional[int] = None,
    control_port: t.Optional[int] = None,
    extensions: t.Optional[t.List[str]] = None,
    default_bridge_type: str = "",
    options: t.Optional[Options] = None,
    capabilities: t.Optional[DesiredCapabilities] = None,
) -> None:
    if pref_dict is None:
        pref_dict = {}
    if extensions is None:
        extensions = []

    self.tor_cfg = tor_cfg
    self.setup_tbb_paths(tbb_path, tbb_fx_binary_path, tbb_profile_path, tor_data_dir)
    self.profile = webdriver.FirefoxProfile(self.tbb_profile_path)
    self.install_extensions(extensions)
    self.init_ports(tor_cfg, socks_port, control_port)
    self.init_prefs(pref_dict, default_bridge_type)
    self.setup_capabilities(capabilities)
    self.export_env_vars()
    self.binary = self.get_tb_binary(logfile=tbb_logfile_path)
    self.binary.add_command_line_options("--class", '"Tor Browser"')
    super(TorBrowserDriver, self).__init__(
        firefox_profile=self.profile,
        firefox_binary=self.binary,
        capabilities=self.capabilities,
        timeout=cm.TB_INIT_TIMEOUT,
        options=options,
        log_path=tbb_logfile_path,
    )
    self.is_running = True
    time.sleep(1)


if __name__ == "__main__":
    main()
