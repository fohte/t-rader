import base64
import importlib.util
import io
import json
import os
import struct
import sys
import tempfile
import unittest
from pathlib import Path


ENTRYPOINT_PATH = Path(__file__).with_name("entrypoint.py")
SPEC = importlib.util.spec_from_file_location("exec_entrypoint", ENTRYPOINT_PATH)
ENTRYPOINT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ENTRYPOINT)


def _frame(data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + data


def _input_stream(manifest: dict, files: list[bytes]) -> bytes:
    encoded = json.dumps(manifest, separators=(",", ":")).encode("utf-8")
    stream = bytearray(ENTRYPOINT.STREAM_MAGIC + _frame(encoded))
    for content in files:
        for offset in range(0, len(content), manifest["limits"]["chunk_bytes"]):
            stream.extend(_frame(content[offset : offset + manifest["limits"]["chunk_bytes"]]))
        stream.extend(_frame(b""))
    return bytes(stream)


def _read_response(stream: bytes) -> tuple[dict, dict[str, bytes]]:
    offset = len(ENTRYPOINT.STREAM_MAGIC)
    header_size = struct.unpack(">I", stream[offset : offset + 4])[0]
    offset += 4
    response = json.loads(stream[offset : offset + header_size])
    offset += header_size

    def read_chunks() -> bytes:
        nonlocal offset
        content = bytearray()
        while True:
            size = struct.unpack(">I", stream[offset : offset + 4])[0]
            offset += 4
            if size == 0:
                return bytes(content)
            content.extend(stream[offset : offset + size])
            offset += size

    stdout_bytes = response.pop("stdout_bytes")
    stderr_bytes = response.pop("stderr_bytes")
    response["stdout_bytes"] = stdout_bytes
    response["stderr_bytes"] = stderr_bytes
    response["stdout"] = read_chunks().decode("utf-8")
    response["stderr"] = read_chunks().decode("utf-8")
    files = {}
    for descriptor in response["files"]:
        files[descriptor["path"]] = read_chunks()
    return response, files


class EntryPointTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory(
            dir=os.environ.get("AGENT_TMPDIR") or None
        )
        self.original_input_root = ENTRYPOINT.INPUT_ROOT
        self.original_output_root = ENTRYPOINT.OUTPUT_ROOT
        self.original_program_path = ENTRYPOINT.PROGRAM_PATH
        ENTRYPOINT.INPUT_ROOT = str(Path(self.temp_dir.name) / "input")
        ENTRYPOINT.OUTPUT_ROOT = str(Path(self.temp_dir.name) / "output")
        ENTRYPOINT.PROGRAM_PATH = str(Path(self.temp_dir.name) / "program.py")

    def tearDown(self) -> None:
        ENTRYPOINT.INPUT_ROOT = self.original_input_root
        ENTRYPOINT.OUTPUT_ROOT = self.original_output_root
        ENTRYPOINT.PROGRAM_PATH = self.original_program_path
        self.temp_dir.cleanup()

    def run_file_transfer(self, request: bytes) -> tuple[dict, dict[str, bytes]]:
        original_stdin, original_stdout = sys.stdin, sys.stdout
        captured = io.BytesIO()
        sys.stdin = io.TextIOWrapper(io.BytesIO(request), encoding="utf-8")
        sys.stdout = io.TextIOWrapper(captured, encoding="utf-8")
        try:
            ENTRYPOINT._run_file_transfer()
            sys.stdout.flush()
            return _read_response(captured.getvalue())
        finally:
            sys.stdin, sys.stdout = original_stdin, original_stdout

    def manifest(self, code: str, files: list[dict]) -> dict:
        return {
            "version": 1,
            "code": code,
            "stdin": "input text",
            "files": files,
            "limits": {
                "max_files": 4,
                "max_file_bytes": 32,
                "max_total_bytes": 64,
                "chunk_bytes": 2,
                "max_output_bytes": 64,
            },
        }

    def test_runs_code_after_receiving_inputs_and_streams_output_files(self) -> None:
        code = (
            "from pathlib import Path\n"
            "import os\n"
            "import sys\n"
            "source = Path(os.environ['KATA_INPUT_DIR'], 'input/sample.bin').read_bytes()\n"
            "Path(os.environ['KATA_OUTPUT_DIR'], 'output').mkdir()\n"
            "Path(os.environ['KATA_OUTPUT_DIR'], 'output/result.bin').write_bytes(source[::-1])\n"
            "print(sys.stdin.read())\n"
        )
        manifest = self.manifest(code, [{"path": "input/sample.bin", "bytes": 4}])
        response, files = self.run_file_transfer(_input_stream(manifest, [b"abcd"]))

        self.assertEqual(
            (response, files),
            (
                {
                    "stdout": "input text\n",
                    "stderr": "",
                    "stdout_bytes": 11,
                    "stderr_bytes": 0,
                    "exit_code": 0,
                    "files": [{"path": "output/result.bin", "bytes": 4}],
                },
                {"output/result.bin": b"dcba"},
            ),
        )

    def test_rejects_path_traversal_before_running_code(self) -> None:
        code = "from pathlib import Path\nimport os\nPath(os.environ['KATA_OUTPUT_DIR'], 'ran').write_text('yes')\n"
        manifest = self.manifest(code, [{"path": "../outside", "bytes": 1}])
        response, files = self.run_file_transfer(_input_stream(manifest, [b"x"]))

        self.assertEqual(
            (response, files),
            (
                {
                    "stdout": "",
                    "stderr": "",
                    "stdout_bytes": 0,
                    "stderr_bytes": 0,
                    "exit_code": 2,
                    "error": "invalid file path",
                    "files": [],
                },
                {},
            ),
        )

    def test_rejects_conflicting_input_paths_before_running_code(self) -> None:
        manifest = self.manifest(
            "print('should not run')",
            [{"path": "item", "bytes": 1}, {"path": "item/child", "bytes": 1}],
        )
        response, files = self.run_file_transfer(_input_stream(manifest, [b"a", b"b"]))

        self.assertEqual(
            (response, files),
            (
                {
                    "stdout": "",
                    "stderr": "",
                    "stdout_bytes": 0,
                    "stderr_bytes": 0,
                    "exit_code": 2,
                    "error": "conflicting file paths",
                    "files": [],
                },
                {},
            ),
        )

    def test_rejects_symlink_outputs(self) -> None:
        code = "from pathlib import Path\nimport os\nPath(os.environ['KATA_OUTPUT_DIR'], 'link').symlink_to('/outside')\n"
        manifest = self.manifest(code, [])
        response, files = self.run_file_transfer(_input_stream(manifest, []))

        self.assertEqual(
            (response, files),
            (
                {
                    "stdout": "",
                    "stderr": "",
                    "stdout_bytes": 0,
                    "stderr_bytes": 0,
                    "exit_code": 2,
                    "error": "output contains a symbolic link",
                    "files": [],
                },
                {},
            ),
        )

    def test_does_not_run_code_when_input_stream_is_truncated(self) -> None:
        code = "from pathlib import Path\nimport os\nPath(os.environ['KATA_OUTPUT_DIR'], 'ran').write_text('yes')\n"
        manifest = self.manifest(code, [{"path": "input/sample.bin", "bytes": 4}])
        request = _input_stream(manifest, [b"ab"])
        response, files = self.run_file_transfer(request[:-4])

        self.assertEqual(
            (response, files),
            (
                {
                    "stdout": "",
                    "stderr": "",
                    "stdout_bytes": 0,
                    "stderr_bytes": 0,
                    "exit_code": 2,
                    "error": "input stream ended early",
                    "files": [],
                },
                {},
            ),
        )

    def test_rejects_text_output_over_the_limit(self) -> None:
        manifest = self.manifest("print('x' * 100)", [])
        manifest["limits"]["max_output_bytes"] = 16
        response, files = self.run_file_transfer(_input_stream(manifest, []))

        self.assertEqual(
            (response, files),
            (
                {
                    "stdout": "",
                    "stderr": "",
                    "stdout_bytes": 0,
                    "stderr_bytes": 0,
                    "exit_code": 2,
                    "error": "text output limit exceeded",
                    "files": [],
                },
                {},
            ),
        )

    def test_rejects_file_count_over_manifest_limit(self) -> None:
        manifest = self.manifest("print('should not run')", [{"path": "sample.bin", "bytes": 1}])
        manifest["limits"]["max_files"] = 0
        response, files = self.run_file_transfer(_input_stream(manifest, [b"x"]))

        self.assertEqual(
            (response, files),
            (
                {
                    "stdout": "",
                    "stderr": "",
                    "stdout_bytes": 0,
                    "stderr_bytes": 0,
                    "exit_code": 2,
                    "error": "invalid transfer limits",
                    "files": [],
                },
                {},
            ),
        )

    def test_rejects_file_bytes_over_manifest_limit(self) -> None:
        manifest = self.manifest("print('should not run')", [{"path": "sample.bin", "bytes": 2}])
        manifest["limits"]["max_file_bytes"] = 1
        response, files = self.run_file_transfer(_input_stream(manifest, [b"xy"]))

        self.assertEqual(
            (response, files),
            (
                {
                    "stdout": "",
                    "stderr": "",
                    "stdout_bytes": 0,
                    "stderr_bytes": 0,
                    "exit_code": 2,
                    "error": "input file exceeds the limit",
                    "files": [],
                },
                {},
            ),
        )

    def test_rejects_output_file_count_over_manifest_limit(self) -> None:
        code = (
            "from pathlib import Path\n"
            "import os\n"
            "root = Path(os.environ['KATA_OUTPUT_DIR'])\n"
            "(root / 'first.bin').write_bytes(b'x')\n"
            "(root / 'second.bin').write_bytes(b'y')\n"
        )
        manifest = self.manifest(code, [])
        manifest["limits"]["max_files"] = 1
        response, files = self.run_file_transfer(_input_stream(manifest, []))

        self.assertEqual(
            (response, files),
            (
                {
                    "stdout": "",
                    "stderr": "",
                    "stdout_bytes": 0,
                    "stderr_bytes": 0,
                    "exit_code": 2,
                    "error": "output file limit exceeded",
                    "files": [],
                },
                {},
            ),
        )

    def test_rejects_output_file_bytes_over_manifest_limit(self) -> None:
        code = (
            "from pathlib import Path\n"
            "import os\n"
            "Path(os.environ['KATA_OUTPUT_DIR'], 'result.bin').write_bytes(b'xy')\n"
        )
        manifest = self.manifest(code, [])
        manifest["limits"]["max_file_bytes"] = 1
        response, files = self.run_file_transfer(_input_stream(manifest, []))

        self.assertEqual(
            (response, files),
            (
                {
                    "stdout": "",
                    "stderr": "",
                    "stdout_bytes": 0,
                    "stderr_bytes": 0,
                    "exit_code": 2,
                    "error": "output file limit exceeded",
                    "files": [],
                },
                {},
            ),
        )

    def test_keeps_legacy_json_evaluation_envelope(self) -> None:
        original_stdout = sys.stdout
        original_code = os.environ.get("EXEC_CODE_B64")
        original_stdin = os.environ.get("EXEC_STDIN_B64")
        captured = io.StringIO()
        os.environ["EXEC_CODE_B64"] = base64.b64encode(b"print('ready')").decode("ascii")
        os.environ["EXEC_STDIN_B64"] = base64.b64encode(b"").decode("ascii")
        sys.stdout = captured
        try:
            ENTRYPOINT._run_legacy()
        finally:
            sys.stdout = original_stdout
            if original_code is None:
                os.environ.pop("EXEC_CODE_B64", None)
            else:
                os.environ["EXEC_CODE_B64"] = original_code
            if original_stdin is None:
                os.environ.pop("EXEC_STDIN_B64", None)
            else:
                os.environ["EXEC_STDIN_B64"] = original_stdin

        self.assertEqual(
            captured.getvalue(),
            ENTRYPOINT.ENVELOPE_MARKER
            + "\n"
            + json.dumps({"stdout": "ready\n", "stderr": "", "exit_code": 0}),
        )


if __name__ == "__main__":
    unittest.main()
