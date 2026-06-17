# syntax=docker/dockerfile:1

# Kata Containers (microVM) 上で LLM 由来の Python コードを 1 回だけ実行するための image。
# backend が動的に作る Pod がこの image を pull して、`EXEC_CODE_B64` / `EXEC_STDIN_B64`
# 環境変数からコードと標準入力を受け取り、JSON envelope を最終行に印字して終了する。
#
# Pod 側で `runAsNonRoot: true`, `readOnlyRootFilesystem: true`,
# `capabilities.drop: ["ALL"]` を強制しているので、container 側もそれを満たす状態にしておく。

FROM python:3.12-slim

RUN groupadd --system --gid 65532 runner \
    && useradd --system --uid 65532 --gid 65532 --no-create-home --shell /usr/sbin/nologin runner

COPY entrypoint.py /usr/local/bin/exec-entrypoint.py
RUN chmod +x /usr/local/bin/exec-entrypoint.py

USER 65532:65532
ENTRYPOINT ["python3", "/usr/local/bin/exec-entrypoint.py"]

LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader
