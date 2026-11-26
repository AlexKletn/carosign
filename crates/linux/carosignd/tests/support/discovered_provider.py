import os
import signal
import socket
import struct
import sys


def encode_bytes(field, value):
    assert len(value) < 128
    return bytes((field << 3 | 2, len(value))) + value


def decode_fields(packet):
    def varint():
        value = 0
        for shift in range(0, 64, 7):
            byte = next(data)
            value |= (byte & 127) << shift
            if not byte & 128:
                return value
        raise ValueError("oversized varint")

    data = iter(packet)
    fields = {}
    for tag in data:
        if tag & 7 == 0:
            fields[tag >> 3] = varint()
        elif tag & 7 == 2:
            fields[tag >> 3] = bytes(next(data) for _ in range(varint()))
        else:
            raise ValueError("unsupported field")
    return fields


def require_platform(connection):
    credentials = connection.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12)
    _, uid, gid = struct.unpack("3i", credentials)
    assert (uid, gid) == (0, 0)


def connect_worker(request, response_prefix, executable, state, authenticator, min_epoch):
    assert request[1] == authenticator
    assert min_epoch <= request[2] < 128
    with socket.socket(socket.AF_UNIX, socket.SOCK_SEQPACKET) as worker:
        worker.connect(request[4].decode())
        require_platform(worker)
        proof = (
            response_prefix
            + encode_bytes(3, authenticator)
            + bytes((0x20, request[2]))
            + encode_bytes(5, request[3])
            + encode_bytes(6, request[5])
        )
        worker.sendall(proof)
        fd = worker.fileno()
        os.set_inheritable(fd, True)
        os.execve(executable, ["mock-worker", str(fd), "linux-persistent"], {
            "CAROSIGN_MOCK_STATE": state,
        })


def main(path, binary, state, uid="65534", authenticator="03" * 16, min_epoch="2"):
    uid = int(uid)
    authenticator = bytes.fromhex(authenticator)
    min_epoch = int(min_epoch)
    executable = os.open(binary, os.O_RDONLY)
    os.setgroups([])
    os.setgid(uid)
    os.setuid(uid)
    signal.signal(signal.SIGCHLD, signal.SIG_IGN)
    capabilities = b"\x08\x01" + encode_bytes(2, bytes((1, 2, 3, 4))) + b"\x50\x01"
    descriptor = encode_bytes(1, authenticator) + encode_bytes(3, capabilities) + b"\x20\x01"
    inventory = encode_bytes(10, encode_bytes(1, descriptor))
    with socket.socket(socket.AF_UNIX, socket.SOCK_SEQPACKET) as listener:
        listener.bind(path)
        listener.listen()
        while True:
            with listener.accept()[0] as control:
                require_platform(control)
                fields = decode_fields(control.recv(65536))
                assert fields[1] == 1 and len(fields[2]) == 16
                prefix = b"\x08\x01" + encode_bytes(2, fields[2])
                if 10 in fields:
                    with open(state + ".described", "a"):
                        pass
                    control.sendall(prefix + inventory)
                    continue
                request = decode_fields(fields[11])
                control.sendall(prefix + encode_bytes(11, b""))
                if os.fork() == 0:
                    try:
                        control.close()
                        listener.close()
                        signal.signal(signal.SIGCHLD, signal.SIG_DFL)
                        connect_worker(request, prefix, executable, state, authenticator, min_epoch)
                    finally:
                        os._exit(1)


if __name__ == "__main__":
    main(*sys.argv[1:])
