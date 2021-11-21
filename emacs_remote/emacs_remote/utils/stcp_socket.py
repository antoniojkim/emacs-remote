import select
import socket
import zlib
from dataclasses import astuple, is_dataclass

import msgpack


class SecureTCPSocket:
    def __init__(self, s=None):
        if s is None:
            s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

        self.socket = s

    def __enter__(self):
        self.socket.__enter__()
        return self

    def __exit__(self, *args):
        self.socket.__exit__(*args)

    def bind(self, host, port):
        return self.socket.bind((host, port))

    def listen(self):
        return self.socket.listen()

    def accept(self):
        conn, addr = self.socket.accept()
        return SecureTCPSocket(conn), addr

    def connect(self, host, port):
        return self.socket.connect((host, port))

    def sendall(self, data):
        message_type = get_message_type(data)

        if isinstance(data, str):
            data = data.encode("utf-8")
        elif is_dataclass(data):
            data = astuple(data)
        elif not isinstance(data, (list, tuple, dict)):
            raise TypeError(
                f"Expected data to be one of [str, list, tuple, dict, dataclass]. Got {type(data)}"
            )

        packed = msgpack.packb(data)
        compressed = zlib.compress(packed)

        size_message = msgpack.packb((message_type, len(compressed)))
        self.socket.sendall(size_message)
        self.socket.sendall(compressed)

    def recvall(self, timeout: float = None):
        """
        Receive all bytes in a message

        Will recv all for 2 messages. First one denoting size and type of actual message.
        Second one being the actual payload.

        Args:
            timeout: positive floating value representing seconds after which to return None
                timeout only applies to waiting for first message. Not second.
        """
        if timeout is not None:
            assert isinstance(timeout, float) and timeout > 0

            self.socket.setblocking(0)
            ready = select.select([self.socket], [], [], timeout)
            if not ready[0]:
                return None

        data = self.socket.recv(1024)
        message_type, message_size = msgpack.unpackb(data)

        data = bytearray()
        while len(data) < message_size:
            data.extend(self.socket.recv(1024))

        data = zlib.decompress(data)
        data = msgpack.unpackb(data)

        return get_message(message_type, data)