"""Kata exec Pod の entrypoint.

backend との contract:
- stdout の最終出現 `__T_RADER_ENVELOPE__\\n<json>` を envelope として取り出す。
- envelope の shape は {"stdout": str, "stderr": str, "exit_code": int}。
"""

import base64
import io
import json
import os
import sys
import traceback

ENVELOPE_MARKER = "__T_RADER_ENVELOPE__"


def _decode(name: str) -> str:
    raw = os.environ.get(name, "")
    if not raw:
        return ""
    return base64.b64decode(raw).decode("utf-8", errors="replace")


def main() -> None:
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
        except SystemExit as e:
            if isinstance(e.code, int):
                exit_code = e.code
            elif e.code is None:
                exit_code = 0
            else:
                stderr_buf.write(str(e.code) + "\n")
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


if __name__ == "__main__":
    main()
