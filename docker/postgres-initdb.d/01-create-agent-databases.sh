#!/bin/sh
# postgres 公式イメージが初回起動時 (data ディレクトリが空のとき) にのみ実行する
# docker-entrypoint-initdb.d フック。agent サービスは backend とは別の論理 DB を
# 同じ Postgres インスタンス上に持つ。
set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<- EOSQL
    CREATE DATABASE t_rader_agent_development;
    CREATE DATABASE t_rader_agent_test;
EOSQL
