"""Kata exec Pod の固定 wrapper。

従来の小さい JSON 評価は環境変数と最終行の envelope を使う。ファイル評価は
stdin / stdout の attach stream で manifest と有界 chunk を転送する。
"""

import base64
import codecs
import io
import json
import os
import selectors
import signal
import stat
import struct
import subprocess
import sys
import traceback

ENVELOPE_MARKER = "__T_RADER_ENVELOPE__"
STREAM_MAGIC = b"T-RADER-FILE-TRANSFER-1\n"
MAX_MANIFEST_BYTES = 1024 * 1024
INPUT_ROOT = "/tmp/kata-input"
OUTPUT_ROOT = "/tmp/kata-output"
PROGRAM_PATH = "/tmp/kata-program.py"
MAX_FILE_COUNT = 128
MAX_FILE_BYTES = 8 * 1024 * 1024
MAX_TOTAL_FILE_BYTES = 16 * 1024 * 1024
MAX_CHUNK_BYTES = 64 * 1024
MAX_OUTPUT_BYTES = 8 * 1024 * 1024


class TransferError(Exception):
    pass


def _decode(name: str) -> str:
    raw = os.environ.get(name, "")
    if not raw:
        return ""
    return base64.b64decode(raw).decode("utf-8", errors="replace")


def _read_exact(stream, size: int) -> bytes:
    chunks = bytearray()
    while len(chunks) < size:
        chunk = stream.read(size - len(chunks))
        if not chunk:
            raise TransferError("input stream ended early")
        chunks.extend(chunk)
    return bytes(chunks)


def _read_u32(stream) -> int:
    return struct.unpack(">I", _read_exact(stream, 4))[0]


def _validate_relative_path(path: str) -> list[str]:
    if not isinstance(path, str) or not path or "\\" in path or "\0" in path:
        raise TransferError("invalid file path")
    parts = path.split("/")
    if (
        len(path.encode("utf-8")) > 4096
        or any(
            part in ("", ".", "..") or len(part.encode("utf-8")) > 255
            for part in parts
        )
    ):
        raise TransferError("invalid file path")
    return parts


def _mkdir_parents(root_fd: int, parts: list[str]) -> int:
    current_fd = os.dup(root_fd)
    for part in parts:
        try:
            os.mkdir(part, mode=0o700, dir_fd=current_fd)
        except FileExistsError:
            pass
        next_fd = os.open(
            part,
            os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
            dir_fd=current_fd,
        )
        os.close(current_fd)
        current_fd = next_fd
    return current_fd


def _open_relative_file(root_fd: int, path: str, flags: int) -> int:
    parts = _validate_relative_path(path)
    parent_fd = os.dup(root_fd)
    try:
        for part in parts[:-1]:
            next_fd = os.open(
                part,
                os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                dir_fd=parent_fd,
            )
            os.close(parent_fd)
            parent_fd = next_fd
        return os.open(parts[-1], flags | os.O_NOFOLLOW, dir_fd=parent_fd)
    finally:
        os.close(parent_fd)


def _read_manifest(stream) -> dict:
    if _read_exact(stream, len(STREAM_MAGIC)) != STREAM_MAGIC:
        raise TransferError("invalid stream header")
    size = _read_u32(stream)
    if size > MAX_MANIFEST_BYTES:
        raise TransferError("manifest exceeds the limit")
    try:
        manifest = json.loads(_read_exact(stream, size))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise TransferError("invalid manifest") from error
    if not isinstance(manifest, dict) or manifest.get("version") != 1:
        raise TransferError("unsupported manifest")
    limits = manifest.get("limits")
    files = manifest.get("files")
    if not isinstance(limits, dict) or not isinstance(files, list):
        raise TransferError("invalid manifest")
    for key in ("max_files", "max_file_bytes", "max_total_bytes", "chunk_bytes", "max_output_bytes"):
        if not isinstance(limits.get(key), int) or limits[key] < 0:
            raise TransferError("invalid transfer limits")
    if (
        limits["chunk_bytes"] == 0
        or limits["chunk_bytes"] > MAX_CHUNK_BYTES
        or limits["max_files"] > MAX_FILE_COUNT
        or limits["max_file_bytes"] > MAX_FILE_BYTES
        or limits["max_total_bytes"] > MAX_TOTAL_FILE_BYTES
        or limits["max_output_bytes"] > MAX_OUTPUT_BYTES
        or len(files) > limits["max_files"]
    ):
        raise TransferError("invalid transfer limits")
    if not isinstance(manifest.get("code"), str) or not isinstance(manifest.get("stdin"), str):
        raise TransferError("invalid manifest")
    return manifest


def _receive_inputs(stream, manifest: dict) -> None:
    limits = manifest["limits"]
    descriptors = manifest["files"]
    paths = set()
    total_bytes = 0
    for descriptor in descriptors:
        if not isinstance(descriptor, dict):
            raise TransferError("invalid file manifest")
        path = descriptor.get("path")
        _validate_relative_path(path)
        size = descriptor.get("bytes")
        if not isinstance(size, int) or size < 0 or size > limits["max_file_bytes"]:
            raise TransferError("input file exceeds the limit")
        if path in paths or any(
            path.startswith(existing + "/") or existing.startswith(path + "/")
            for existing in paths
        ):
            raise TransferError("conflicting file paths")
        paths.add(path)
        total_bytes += size
        if total_bytes > limits["max_total_bytes"]:
            raise TransferError("input files exceed the limit")

    root_fd = os.open(INPUT_ROOT, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for descriptor in descriptors:
            path = descriptor.get("path")
            size = descriptor.get("bytes")
            parts = _validate_relative_path(path)
            parent_fd = _mkdir_parents(root_fd, parts[:-1])
            try:
                file_fd = os.open(
                    parts[-1],
                    os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                    0o600,
                    dir_fd=parent_fd,
                )
                received = 0
                with os.fdopen(file_fd, "wb") as target:
                    while True:
                        chunk_size = _read_u32(stream)
                        if chunk_size == 0:
                            break
                        if chunk_size > limits["chunk_bytes"]:
                            raise TransferError("input chunk exceeds the limit")
                        received += chunk_size
                        if received > size:
                            raise TransferError("input file exceeds its declared size")
                        target.write(_read_exact(stream, chunk_size))
                if received != size:
                    raise TransferError("input file size did not match its manifest")
            finally:
                os.close(parent_fd)
    finally:
        os.close(root_fd)


def _scan_output_directory(root_fd: int, limits: dict) -> list[dict]:
    files = []
    total_bytes = 0

    def visit(directory_fd: int, prefix: str) -> None:
        nonlocal total_bytes
        for name in sorted(os.listdir(directory_fd)):
            path = f"{prefix}/{name}" if prefix else name
            _validate_relative_path(path)
            info = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
            if stat.S_ISLNK(info.st_mode):
                raise TransferError("output contains a symbolic link")
            if stat.S_ISDIR(info.st_mode):
                child_fd = os.open(
                    name,
                    os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW,
                    dir_fd=directory_fd,
                )
                try:
                    visit(child_fd, path)
                finally:
                    os.close(child_fd)
                continue
            if not stat.S_ISREG(info.st_mode):
                raise TransferError("output contains an unsupported file type")
            if len(files) >= limits["max_files"] or info.st_size > limits["max_file_bytes"]:
                raise TransferError("output file limit exceeded")
            total_bytes += info.st_size
            if total_bytes > limits["max_total_bytes"]:
                raise TransferError("output byte limit exceeded")
            files.append({"path": path, "bytes": info.st_size})

    visit(root_fd, "")
    return files


def _write_u32(stream, value: int) -> None:
    stream.write(struct.pack(">I", value))


def _write_chunked_bytes(stream, data: bytes, chunk_bytes: int) -> None:
    for offset in range(0, len(data), chunk_bytes):
        chunk = data[offset : offset + chunk_bytes]
        _write_u32(stream, len(chunk))
        stream.write(chunk)
    _write_u32(stream, 0)


def _read_output_files(files: list[dict]) -> list[bytes]:
    root_fd = os.open(OUTPUT_ROOT, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    contents = []
    try:
        for descriptor in files:
            file_fd = _open_relative_file(
                root_fd, descriptor["path"], os.O_RDONLY | os.O_NONBLOCK
            )
            try:
                if not stat.S_ISREG(os.fstat(file_fd).st_mode):
                    raise TransferError("output contains an unsupported file type")
                content = bytearray()
                while len(content) <= descriptor["bytes"]:
                    chunk = os.read(
                        file_fd,
                        min(MAX_CHUNK_BYTES, descriptor["bytes"] + 1 - len(content)),
                    )
                    if not chunk:
                        break
                    content.extend(chunk)
                if len(content) != descriptor["bytes"]:
                    raise TransferError("output file changed during transfer")
                contents.append(bytes(content))
            finally:
                os.close(file_fd)
    finally:
        os.close(root_fd)
    return contents


def _prepare_transfer_response(
    response: dict, files: list[dict], file_contents: list[bytes]
) -> tuple[bytes, list[bytes]]:
    stdout = response.get("stdout", "").encode("utf-8")
    stderr = response.get("stderr", "").encode("utf-8")
    payload = {key: value for key, value in response.items() if key not in ("stdout", "stderr")}
    payload["stdout_bytes"] = len(stdout)
    payload["stderr_bytes"] = len(stderr)
    payload["files"] = files
    encoded = json.dumps(payload, separators=(",", ":")).encode("utf-8")
    if len(encoded) > MAX_MANIFEST_BYTES:
        raise TransferError("response manifest exceeds the limit")
    return encoded, [stdout, stderr, *file_contents]


def _write_transfer_response(
    encoded: bytes, payloads: list[bytes], chunk_bytes: int
) -> None:
    output = sys.stdout.buffer
    output.write(STREAM_MAGIC)
    _write_u32(output, len(encoded))
    output.write(encoded)
    for payload in payloads:
        _write_chunked_bytes(output, payload, chunk_bytes)
    output.flush()


def _execute_code(code: str, stdin_text: str, max_output_bytes: int) -> dict:
    program_fd = os.open(
        PROGRAM_PATH,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
        0o600,
    )
    with os.fdopen(program_fd, "w", encoding="utf-8") as program:
        program.write(code)

    environment = os.environ.copy()
    environment["KATA_INPUT_DIR"] = INPUT_ROOT
    environment["KATA_OUTPUT_DIR"] = OUTPUT_ROOT
    try:
        process = subprocess.Popen(
            [sys.executable, "-I", "-X", "utf8", PROGRAM_PATH],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            bufsize=0,
            env=environment,
            start_new_session=True,
        )
    except OSError as error:
        raise TransferError("failed to execute code") from error

    selector = selectors.DefaultSelector()
    stdout_output = bytearray()
    stderr_output = bytearray()
    decoders = {
        "stdout": codecs.getincrementaldecoder("utf-8")("replace"),
        "stderr": codecs.getincrementaldecoder("utf-8")("replace"),
    }
    output_bytes = 0
    input_bytes = stdin_text.encode("utf-8")
    input_offset = 0

    def collect(name: str, data: bytes, final: bool = False) -> bool:
        nonlocal output_bytes
        text = decoders[name].decode(data, final=final)
        encoded_length = len(text.encode("utf-8"))
        if output_bytes + encoded_length > max_output_bytes:
            return False
        output_bytes += encoded_length
        if text:
            (stdout_output if name == "stdout" else stderr_output).extend(
                text.encode("utf-8")
            )
        return True

    def kill_process_group() -> None:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass

    try:
        for name, pipe in (("stdout", process.stdout), ("stderr", process.stderr)):
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, name)
        stdin_pipe = process.stdin
        stdin_fd = stdin_pipe.fileno()
        if input_bytes:
            os.set_blocking(stdin_fd, False)
            selector.register(stdin_pipe, selectors.EVENT_WRITE, "stdin")
        else:
            stdin_pipe.close()

        while selector.get_map():
            if process.poll() is not None and stdin_fd in selector.get_map():
                selector.unregister(stdin_pipe)
                stdin_pipe.close()
            for key, _ in selector.select(timeout=0.1):
                if key.data == "stdin":
                    try:
                        input_offset += os.write(
                            stdin_fd, input_bytes[input_offset : input_offset + 65536]
                        )
                    except BlockingIOError:
                        continue
                    except BrokenPipeError:
                        selector.unregister(stdin_pipe)
                        stdin_pipe.close()
                        continue
                    if input_offset == len(input_bytes):
                        selector.unregister(stdin_pipe)
                        stdin_pipe.close()
                    continue

                name = key.data
                try:
                    data = os.read(key.fd, 65536)
                except BlockingIOError:
                    continue
                if not data:
                    selector.unregister(key.fileobj)
                    key.fileobj.close()
                    if not collect(name, b"", final=True):
                        kill_process_group()
                        process.wait()
                        return {
                            "stdout": "",
                            "stderr": "",
                            "exit_code": 2,
                            "error": "text output limit exceeded",
                        }
                    continue
                if not collect(name, data):
                    kill_process_group()
                    process.wait()
                    return {
                        "stdout": "",
                        "stderr": "",
                        "exit_code": 2,
                        "error": "text output limit exceeded",
                    }

        return {
            "stdout": stdout_output.decode("utf-8"),
            "stderr": stderr_output.decode("utf-8"),
            "exit_code": process.wait(),
        }
    except OSError as error:
        raise TransferError("failed to execute code") from error
    finally:
        selector.close()
        for pipe in (process.stdin, process.stdout, process.stderr):
            if pipe is not None and not pipe.closed:
                pipe.close()
        if process.poll() is None:
            kill_process_group()
            process.wait()


def _run_file_transfer() -> None:
    response = {"stdout": "", "stderr": "", "exit_code": 2}
    limits = {"chunk_bytes": 1}
    try:
        os.mkdir(INPUT_ROOT, mode=0o700)
        os.mkdir(OUTPUT_ROOT, mode=0o700)
        manifest = _read_manifest(sys.stdin.buffer)
        _receive_inputs(sys.stdin.buffer, manifest)
        limits = manifest["limits"]
        response = _execute_code(
            manifest["code"], manifest["stdin"], limits["max_output_bytes"]
        )
        if response.get("error"):
            raise TransferError(response["error"])
        root_fd = os.open(OUTPUT_ROOT, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            files = _scan_output_directory(root_fd, limits)
        finally:
            os.close(root_fd)
        file_contents = _read_output_files(files)
        encoded, payloads = _prepare_transfer_response(response, files, file_contents)
    except Exception as error:
        message = str(error) if isinstance(error, TransferError) else "file transfer failed"
        response = {"stdout": "", "stderr": "", "exit_code": 2, "error": message}
        files = []
        encoded, payloads = _prepare_transfer_response(response, files, [])
    _write_transfer_response(encoded, payloads, limits["chunk_bytes"])


def _run_legacy() -> None:
    stdout_buf = io.StringIO()
    stderr_buf = io.StringIO()
    exit_code = 0

    code = _decode("EXEC_CODE_B64")
    if not code:
        stderr_buf.write("EXEC_CODE_B64 is not set\n")
        exit_code = 2
    else:
        stdin_text = _decode("EXEC_STDIN_B64")
        old_stdout, old_stderr, old_stdin = sys.stdout, sys.stderr, sys.stdin
        sys.stdout = stdout_buf
        sys.stderr = stderr_buf
        sys.stdin = io.StringIO(stdin_text)
        try:
            compiled = compile(code, "<exec>", "exec")
            exec(compiled, {"__name__": "__main__"})
        except SystemExit as error:
            if isinstance(error.code, int):
                exit_code = error.code
            elif error.code is None:
                exit_code = 0
            else:
                stderr_buf.write(str(error.code) + "\n")
                exit_code = 1
        except BaseException:
            traceback.print_exc(file=stderr_buf)
            exit_code = 1
        finally:
            sys.stdout, sys.stderr, sys.stdin = old_stdout, old_stderr, old_stdin

    envelope = {
        "stdout": stdout_buf.getvalue(),
        "stderr": stderr_buf.getvalue(),
        "exit_code": exit_code,
    }
    sys.stdout.write(ENVELOPE_MARKER + "\n")
    sys.stdout.write(json.dumps(envelope))
    sys.stdout.flush()


def main() -> None:
    if os.environ.get("EXEC_MODE") == "file-transfer":
        _run_file_transfer()
    else:
        _run_legacy()


if __name__ == "__main__":
    main()
