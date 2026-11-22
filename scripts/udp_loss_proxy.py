# SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
# SPDX-License-Identifier: MIT

"""Local UDP relay that introduces deterministic downstream loss after handshake."""
import socket
import threading
import time


class LossProxy:
    def __init__(self, listen_port, server_port, every):
        self.server = ("127.0.0.1", server_port)
        self.every = every
        self.dropped = 0
        self.forwarded = 0
        self.client = None
        self.stopped = threading.Event()
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.socket.bind(("127.0.0.1", listen_port))
        self.socket.settimeout(0.1)
        self.started = time.monotonic()
        self.thread = threading.Thread(target=self.run, daemon=True)
        self.thread.start()

    def run(self):
        downstream = 0
        while not self.stopped.is_set():
            try:
                data, address = self.socket.recvfrom(65535)
            except (socket.timeout, ConnectionResetError):
                continue
            if address == self.server:
                if self.client is None:
                    continue
                downstream += 1
                if time.monotonic() - self.started > 2 and downstream % self.every == 0:
                    self.dropped += 1
                    continue
                destination = self.client
            else:
                self.client = address
                destination = self.server
            self.socket.sendto(data, destination)
            self.forwarded += 1

    def close(self):
        self.stopped.set()
        self.thread.join(timeout=2)
        self.socket.close()
