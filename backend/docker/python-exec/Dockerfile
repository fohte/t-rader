# syntax=docker/dockerfile:1

# 純粋関数評価のための Kata exec ランタイム image。backend が動的に作る Pod が
# pull して 1 回だけ実行し、`EXEC_CODE_B64` / `EXEC_STDIN_B64` 環境変数から
# コードと標準入力を受け取り、JSON envelope を最終行に印字して終了する。
#
# 同梱ライブラリは数値計算系のホワイトリスト (numpy / pandas) に絞る。
# I/O 系 (requests / urllib3 等) は同梱しない — 通信意図のコードを早期に弾く。
#
# Pod 側で `runAsNonRoot: true`, `readOnlyRootFilesystem: true`,
# `capabilities.drop: ["ALL"]` を強制しているので、container 側もそれを満たす状態にしておく。

FROM python:3.12-slim

# pure-function workload に必要な数値計算ライブラリのみホワイトリスト。
RUN pip install --no-cache-dir \
    numpy==2.1.3 \
    pandas==2.2.3

RUN groupadd --system --gid 65532 runner \
    && useradd --system --uid 65532 --gid 65532 --no-create-home --shell /usr/sbin/nologin runner

COPY entrypoint.py /usr/local/bin/exec-entrypoint.py
RUN chmod +x /usr/local/bin/exec-entrypoint.py

USER 65532:65532
ENTRYPOINT ["python3", "/usr/local/bin/exec-entrypoint.py"]

LABEL org.opencontainers.image.source=https://github.com/fohte/t-rader
