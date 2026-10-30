# -*- coding: utf-8 -*-
"""
==============================================================================
🐍 NEXUS-KV OFFICIAL PYTHON CLIENT LIBRARY & DRIVER
Author: Ali Hasanifard
License: MIT
==============================================================================
"""

import socket
import time

class NexusError(Exception):
    pass

class NexusConnectionError(NexusError):
    pass

class NexusClient:
    """Official high-performance Python client for NexusKV distributed storage engine."""

    def __init__(self, host="127.0.0.1", port=6379, timeout=3.0, auto_reconnect=True):
        self.host = host
        self.port = port
        self.timeout = timeout
        self.auto_reconnect = auto_reconnect
        self._sock = None
        self._connect()

    def _connect(self):
        try:
            self._sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            self._sock.settimeout(self.timeout)
            self._sock.connect((self.host, self.port))
        except Exception as e:
            self._sock = None
            raise NexusConnectionError(f"Could not connect to NexusKV at {self.host}:{self.port}: {e}")

    def _ensure_connection(self):
        if self._sock is None:
            if self.auto_reconnect:
                self._connect()
            else:
                raise NexusConnectionError("Disconnected from NexusKV")

    def _send_command(self, *args):
        self._ensure_connection()

        # Build RESP Array
        payload = f"*{len(args)}\r\n"
        for arg in args:
            if isinstance(arg, str):
                b = arg.encode('utf-8')
            elif isinstance(arg, bytes):
                b = arg
            else:
                b = str(arg).encode('utf-8')
            payload += f"${len(b)}\r\n{b.decode('utf-8', errors='ignore')}\r\n"

        try:
            self._sock.sendall(payload.encode('utf-8'))
            return self._read_response()
        except Exception as e:
            self._sock = None
            if self.auto_reconnect:
                self._connect()
                self._sock.sendall(payload.encode('utf-8'))
                return self._read_response()
            raise NexusConnectionError(f"Socket communication failed: {e}")

    def _read_line(self):
        buf = bytearray()
        while True:
            ch = self._sock.recv(1)
            if not ch:
                raise NexusConnectionError("Connection closed by peer")
            if ch == b'\n' and buf.endswith(b'\r'):
                return buf[:-1].decode('utf-8', errors='ignore')
            buf.extend(ch)

    def _read_response(self):
        prefix = self._sock.recv(1)
        if not prefix:
            raise NexusConnectionError("Empty response from server")

        if prefix == b'+':
            return self._read_line()
        elif prefix == b':':
            return int(self._read_line())
        elif prefix == b'-':
            err_msg = self._read_line()
            raise NexusError(err_msg)
        elif prefix == b'$':
            length = int(self._read_line())
            if length == -1:
                return None
            data = bytearray()
            while len(data) < length:
                chunk = self._sock.recv(min(4096, length - len(data)))
                if not chunk:
                    break
                data.extend(chunk)
            self._sock.recv(2) # consume trailing \r\n
            return data.decode('utf-8', errors='replace')
        elif prefix == b'*':
            count = int(self._read_line())
            if count == -1:
                return None
            items = []
            for _ in range(count):
                items.append(self._read_response())
            return items
        else:
            raise NexusError(f"Unknown RESP response type prefix: {prefix}")

    def ping(self) -> str:
        return self._send_command("PING")

    def set(self, key: str, value: str) -> bool:
        res = self._send_command("SET", key, value)
        return res == "OK"

    def get(self, key: str) -> str | None:
        return self._send_command("GET", key)

    def delete(self, key: str) -> bool:
        res = self._send_command("DEL", key)
        return res == 1

    def keys(self, pattern: str = "*") -> list[str]:
        res = self._send_command("KEYS", pattern)
        return res if isinstance(res, list) else []

    def scan(self, cursor: int = 0, pattern: str = "*") -> tuple[int, list[str]]:
        res = self._send_command("SCAN", str(cursor), pattern)
        if isinstance(res, list) and len(res) == 2:
            return (int(res[0]), res[1])
        return (0, [])

    def role(self) -> str:
        return self._send_command("ROLE")

    def metrics(self) -> str:
        return self._send_command("METRICS")

    def flushall(self) -> bool:
        res = self._send_command("FLUSHALL")
        return res == "OK"

    def expire(self, key: str, seconds: int) -> bool:
        res = self._send_command("EXPIRE", key, str(seconds))
        return res == 1

    def ttl(self, key: str) -> int:
        return int(self._send_command("TTL", key))

    def persist(self, key: str) -> bool:
        res = self._send_command("PERSIST", key)
        return res == 1

    def set_ex(self, key: str, value: str, seconds: int) -> bool:
        res = self._send_command("SET", key, value, "EX", str(seconds))
        return res == "OK"

    def close(self):
        if self._sock:
            try:
                self._sock.close()
            except Exception:
                pass
            self._sock = None

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.close()
