#!/usr/bin/env python3
"""scripts/pair-radio.py
Pair a MeshCore or Meshtastic radio with BlueZ, typing the PIN its screen shows when
bluetoothctl asks for it. A bond made without the PIN (KDE settings, or no agent) is
dropped by the radio a few seconds after each connection.

Usage: scripts/pair-radio.py [address]
With no address, the first radio named "MeshCore-..." or "Meshtastic..." found.

Author: O. Booklage
Date: October 2026
Licence: GPL-3.0
"""

import os
import pty
import re
import select
import sys
import time

RADIO = re.compile(r"Device ([0-9A-F:]{17}) (MeshCore-\S+|Meshtastic\S*)")
NOISE = re.compile(r"\x1b\[[0-9;]*[A-Za-z]|\x01|\x02|\r")


class Bluetoothctl:
    """bluetoothctl in a pseudo-terminal, so its prompts come as they do on screen."""

    def __init__(self):
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.execvp("bluetoothctl", ["bluetoothctl"])
        self.seen = ""

    def send(self, line):
        os.write(self.fd, (line + "\n").encode())

    def expect(self, patterns, timeout):
        """Wait for one of `patterns`; return (its index, the match), or (None, None)."""
        end = time.time() + timeout
        while time.time() < end:
            for i, pattern in enumerate(patterns):
                match = re.search(pattern, self.seen)
                if match:
                    self.seen = self.seen[match.end():]
                    return i, match
            ready, _, _ = select.select([self.fd], [], [], 0.5)
            if ready:
                try:
                    self.seen += NOISE.sub("", os.read(self.fd, 4096).decode("utf-8", "replace"))
                except OSError:  # bluetoothctl quit
                    break
        return None, None

    def ask(self, command, pattern, timeout=5):
        """Send `command` and wait for `pattern` in what follows."""
        self.seen = ""
        self.send(command)
        return self.expect([pattern], timeout)[1]

    def output(self, command, seconds=1.5):
        """Everything `command` prints within `seconds`."""
        self.seen = ""
        self.send(command)
        self.expect([r"(?!x)x"], seconds)  # matches nothing: just read
        return self.seen


def known(ctl, addr):
    """BlueZ knows the radio now (`info` fails on one it forgot)."""
    return "Name:" in ctl.output(f"info {addr}", 1)


def find(ctl, address):
    """The radio's address and name, once the scan shows it and BlueZ knows it."""
    for _ in range(60):
        text = ctl.output("devices")
        for addr, name in RADIO.findall(text):
            if address in (None, addr) and known(ctl, addr):
                return addr, name
        if address and address in text and known(ctl, address):
            return address, address
        time.sleep(1)
    return None, None


def main():
    address = sys.argv[1].upper() if len(sys.argv) > 1 else None
    ctl = Bluetoothctl()
    ctl.expect([r"#"], 5)
    for command in ("power on", "agent KeyboardOnly", "default-agent", "scan on"):
        ctl.send(command)
    print("Looking for the radio...")
    addr, name = find(ctl, address)
    if not addr:
        sys.exit("No MeshCore or Meshtastic radio found. Is it on, and near enough?")
    # An old bond goes first: the radio would not take a new one on top of it.
    if ctl.ask(f"info {addr}", r"Paired: yes", timeout=3):
        print(f"Removing the old pairing of {name}...")
        ctl.send(f"remove {addr}")
        time.sleep(2)
        # The scan must see it again before it can be paired.
        addr, name = find(ctl, addr)
        if not addr:
            sys.exit("The radio did not show again after removing the old pairing.")
    print(f"Pairing with {name} ({addr})...")
    ctl.seen = ""
    ctl.send(f"pair {addr}")
    prompts = [
        r"Enter (?:passkey|PIN code)[^:]*:",
        r"Confirm passkey (\d+)",
        r"Accept pairing[^:]*:",
    ]
    ends = [r"Pairing successful", r"Failed to pair: (\S+)", r"(not available)"]
    asked = pin = False
    while True:
        # Once answered, bluetoothctl redraws its prompt: only the outcome matters then.
        patterns = ends if asked else prompts + ends
        i, match = ctl.expect(patterns, 90)
        if i is not None and not asked and i < len(prompts):
            if i == 0:
                ctl.send(input("PIN shown on the radio's screen: ").strip())
            elif i == 1:
                answer = input(f"Does the radio show {match.group(1)}? [y/n] ").strip()
                ctl.send("yes" if answer.lower().startswith("y") else "no")
            else:
                ctl.send("yes")
            asked, pin = True, i < 2
            ctl.seen = ""
            continue
        if i is not None and patterns[i] == ends[0]:
            break
        reason = match.group(1) if match else "no answer from the radio"
        ctl.send("quit")
        sys.exit(f"Pairing failed: {reason}")
    ctl.send("scan off")
    ctl.send(f"trust {addr}")
    ctl.expect([r"trust succeeded"], 5)
    ctl.send("quit")
    print(f"Paired with {name}.")
    if not pin:
        print(
            "Warning: no PIN was asked. A MeshCore radio drops such a pairing; check that"
            " bluetoothctl asked for one, or the radio's pairing mode."
        )


if __name__ == "__main__":
    main()
