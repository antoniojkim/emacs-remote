#!/usr/bin/env python3

import logging
import os
import random
import signal
import socket
import subprocess
import sys
from time import sleep
from pathlib import Path
from threading import Event
from queue import Queue, Empty as EmptyQueue

import pexpect

from .. import utils
from ..messages.message import Response
from ..messages.shell_command import ShellRequest
from ..messages.startup import SERVER_STARTUP_MSG
from ..utils.stcp import SecureTCP
from ..utils.logging import get_level


class ClientDaemon:
    def __init__(
        self,
        emacs_remote_path: str,
        host: str,
        workspace: str,
        num_clients: int = 1,
        logging_level: str = "info",
    ):
        self.host = host
        self.workspace = workspace
        self.workspace_hash = utils.md5(workspace)

        self.emacs_remote_path = Path(emacs_remote_path)
        self.emacs_remote_path.mkdir(parents=True, exist_ok=True)
        self.workspace_path = self.emacs_remote_path.joinpath(
            "workspaces", self.workspace_hash
        )
        self.workspace_path.mkdir(parents=True, exist_ok=True)
        os.chdir(self.workspace_path.resolve())

        self.num_clients = num_clients

        self.requests = Queue()
        self.requests.put(ShellRequest(["ls"]))
        self.requests.put(ShellRequest(["git", "status"]))

        self.server = None
        self.exceptions = Queue()

        self.terminate_queue = Queue()

        self.logging_level = get_level(logging_level)
        self.file_handler = logging.FileHandler(
            self.workspace_path.joinpath("client.log"), mode="w"
        )
        self.file_handler.setLevel(self.logging_level)
        logging.basicConfig(
            level=self.logging_level,
            format="%(asctime)s %(name)-12s %(levelname)-8s %(message)s",
            datefmt="%m-%d %H:%M",
        )

    def handle_request(self, request, socket):
        socket.sendall(request)
        data = socket.recvall()

        assert isinstance(data, Response)
        print(data)

    def reset_ssh_connection(self):
        if self.server:
            self.server.stop()

        print(f"Establishing ssh connection with {self.host}...")

        def get_cmd(client_ports, server_ports):
            cmd = []
            # Add args
            cmd.append(f'WORKSPACE="{self.workspace}"')
            cmd.append(f'PORTS="{" ".join(server_ports)}"')

            script_path = Path(sys.prefix, "emacs_remote_scripts", "server.sh")

            cmd.append("bash -s")
            cmd.append("<")
            cmd.append(str(script_path.resolve()))

            return cmd

        def client_handler(index, socket):
            terminate_event = Event()
            self.terminate_queue.put(terminate_event)

            logger = logging.getLogger(f"client.{index}")
            logger.setLevel(self.logging_level)
            logger.addHandler(self.file_handler)
            socket.set_logger(logger)

            while not terminate_event.is_set():
                try:
                    request = self.requests.get(timeout=1)
                    self.handle_request(request, socket)
                except EmptyQueue as e:
                    pass

        def check_started(process):
            for line in process.stdout:
                line = line.decode("utf-8").strip()
                # print(line)
                if line == SERVER_STARTUP_MSG:
                    return True

            return False

        self.server = SecureTCP(self.host, self.num_clients)
        self.server.start(
            get_cmd,
            check_started,
            client_handler,
        )

    def listen(self):
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.bind(("localhost", 0))
            port = s.getsockname()[1]

            daemon_port = self.workspace_path.joinpath("daemon.port")
            daemon_port.write_text(str(port))

            try:
                s.listen()
                conn, addr = s.accept()
                with conn:
                    while True:
                        data = conn.recv(1024)
                        if not data:
                            break
                        conn.sendall(data)
            finally:
                daemon_port.unlink()

    def __enter__(self):
        self.reset_ssh_connection()

        print("Client Daemon Initialized!")
        return self

    def __exit__(self, *args):
        while not self.terminate_queue.empty():
            terminate_event = self.terminate_queue.get()
            terminate_event.set()

        if self.server:
            self.server.stop()
