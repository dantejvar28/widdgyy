#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later
"""Read layout boxes from Firefox headless over the Marionette protocol.

No geckodriver or third-party package is required: Firefox is started with a
temporary profile and `--marionette`, then a tiny length-prefixed JSON client
speaks the protocol directly.
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

SKIP_EXIT = 3


class MarionetteError(RuntimeError):
    pass


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


class Marionette:
    def __init__(self, port: int, timeout: float = 15.0) -> None:
        deadline = time.monotonic() + timeout
        last_error: Exception | None = None
        while time.monotonic() < deadline:
            try:
                self.sock = socket.create_connection(("127.0.0.1", port), timeout=2.0)
                break
            except OSError as error:
                last_error = error
                time.sleep(0.15)
        else:
            raise MarionetteError(f"Marionette port {port} did not open: {last_error}")
        self.sock.settimeout(30.0)
        self.next_id = 1
        hello = self._read()
        if not hello.get("applicationType"):
            raise MarionetteError(f"unexpected Marionette greeting: {hello!r}")

    def _read(self) -> dict:
        header = b""
        while b":" not in header:
            chunk = self.sock.recv(1)
            if not chunk:
                raise MarionetteError("Marionette connection closed")
            header += chunk
        size = int(header[:-1])
        payload = b""
        while len(payload) < size:
            chunk = self.sock.recv(size - len(payload))
            if not chunk:
                raise MarionetteError("Marionette connection closed mid-message")
            payload += chunk
        return json.loads(payload)

    def command(self, name: str, params: dict | None = None) -> dict:
        message_id = self.next_id
        self.next_id += 1
        payload = json.dumps([0, message_id, name, params or {}]).encode()
        self.sock.sendall(str(len(payload)).encode() + b":" + payload)
        reply = self._read()
        if len(reply) < 4:
            raise MarionetteError(f"malformed Marionette reply: {reply!r}")
        error = reply[2]
        result = reply[3]
        if error:
            raise MarionetteError(f"{name}: {error.get('message', error)}")
        return result or {}

    def close(self) -> None:
        try:
            self.sock.close()
        except OSError:
            pass


def collect_script() -> str:
    return r"""
const done = arguments[arguments.length - 1];
const finish = () => {
  const boxes = {};
  for (const element of document.querySelectorAll("[data-test]")) {
    const rect = element.getBoundingClientRect();
    boxes[element.dataset.test] = {
      x: rect.x + window.scrollX,
      y: rect.y + window.scrollY,
      width: rect.width,
      height: rect.height,
    };
  }
  const lines = {};
  for (const element of document.querySelectorAll("[data-lines]")) {
    const range = document.createRange();
    range.selectNodeContents(element);
    lines[element.dataset.lines] = Array.from(range.getClientRects()).map((rect) => ({
      x: rect.x + window.scrollX,
      y: rect.y + window.scrollY,
      width: rect.width,
      height: rect.height,
    }));
  }
  done({
    viewport: {
      width: window.innerWidth,
      height: window.innerHeight,
      devicePixelRatio: window.devicePixelRatio || 1,
    },
    boxes,
    lines,
  });
};
if (document.fonts && document.fonts.ready) {
  document.fonts.ready.then(finish).catch(finish);
} else {
  finish();
}
"""


def run(args: argparse.Namespace) -> int:
    firefox = shutil.which(os.environ.get("FIREFOX", "firefox"))
    if not firefox:
        print("firefox not found; skipping conformance", file=sys.stderr)
        return SKIP_EXIT

    html = Path(args.html).resolve()
    if not html.is_file():
        print(f"missing fixture HTML: {html}", file=sys.stderr)
        return 2

    port = free_port()
    profile = Path(tempfile.mkdtemp(prefix="widdgyy-ff-"))
    (profile / "prefs.js").write_text(
        "\n".join(
            [
                f'user_pref("marionette.port", {port});',
                'user_pref("marionette.force-local", true);',
                'user_pref("browser.shell.checkDefaultBrowser", false);',
                'user_pref("browser.startup.homepage_override.mstone", "ignore");',
                'user_pref("datareporting.policy.dataSubmissionEnabled", false);',
                'user_pref("toolkit.telemetry.reportingpolicy.firstRun", false);',
            ]
        )
        + "\n"
    )

    command = [
        firefox,
        "--headless",
        "--no-remote",
        "--profile",
        str(profile),
        "--marionette",
        "--window-size",
        f"{args.width},{args.height}",
        "about:blank",
    ]
    process = subprocess.Popen(
        command,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )

    client: Marionette | None = None
    try:
        client = Marionette(port)
        client.command("WebDriver:NewSession", {"capabilities": {}})
        client.command(
            "WebDriver:SetWindowRect",
            {"width": int(args.width), "height": int(args.height)},
        )
        client.command("WebDriver:Navigate", {"url": html.as_uri()})
        result = client.command(
            "WebDriver:ExecuteAsyncScript",
            {"script": collect_script(), "args": [], "scriptTimeout": 20000},
        )
        if args.screenshot:
            shot = client.command("WebDriver:TakeScreenshot", {"full": False})
            Path(args.screenshot).write_bytes(base64.b64decode(shot["value"]))
        value = result.get("value", result)
        Path(args.out).write_text(json.dumps(value, indent=2) + "\n")
        return 0
    except MarionetteError as error:
        print(f"Firefox conformance unavailable: {error}", file=sys.stderr)
        return SKIP_EXIT
    finally:
        if client is not None:
            client.close()
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        shutil.rmtree(profile, ignore_errors=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--html", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--width", type=int, default=1280)
    parser.add_argument("--height", type=int, default=720)
    parser.add_argument("--screenshot")
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
