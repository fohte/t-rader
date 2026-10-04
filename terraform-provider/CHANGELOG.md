# Changelog

## [0.1.2](https://github.com/fohte/t-rader/compare/terraform-provider-v0.1.1...terraform-provider-v0.1.2) (2026-10-04)


### Features

* **backend:** RSS 記事本文の保存スキーマとフィード設定を追加する ([#719](https://github.com/fohte/t-rader/issues/719)) ([4079f78](https://github.com/fohte/t-rader/commit/4079f783cc41fee2d6bc80fb8eb96e88cdc6538e))
* **frontend-api:** ジョブが書いたノートを紐付けから取得する ([#715](https://github.com/fohte/t-rader/issues/715)) ([75f6cee](https://github.com/fohte/t-rader/commit/75f6cee99eeb1e0b078bee8677f67299dcea3278))
* **notes:** ノートをタグで絞り込めるようにする ([#721](https://github.com/fohte/t-rader/issues/721)) ([ce44073](https://github.com/fohte/t-rader/commit/ce440730055b528acf7b7c03c661c40ee6e149de))
* **risk-policy:** リスク上限を分類軸ごとに設定する ([#691](https://github.com/fohte/t-rader/issues/691)) ([747e89f](https://github.com/fohte/t-rader/commit/747e89f891b3e87539397f86f693fe0e23c6d097))
* **terraform-provider:** rss_feed の本文取得方式を設定可能にする ([#732](https://github.com/fohte/t-rader/issues/732)) ([e6d7446](https://github.com/fohte/t-rader/commit/e6d7446976aa0de0ea4641734b8140d02793e3aa))

## [0.1.1](https://github.com/fohte/t-rader/compare/terraform-provider-v0.1.0...terraform-provider-v0.1.1) (2026-10-01)


### Features

* **group-axis:** 分類軸の管理 API と Terraform リソースを追加 ([#670](https://github.com/fohte/t-rader/issues/670)) ([9eb6706](https://github.com/fohte/t-rader/commit/9eb670613d689e414239d8bab733630185cc8cd3))
* **terraform-provider:** ノート種別リソースを追加する ([#664](https://github.com/fohte/t-rader/issues/664)) ([6f312c1](https://github.com/fohte/t-rader/commit/6f312c10f766965a881ac050acb14875d27145b9))

## [0.1.0](https://github.com/fohte/t-rader/compare/terraform-provider-v0.1.0...terraform-provider-v0.1.0) (2026-09-30)


### Features

* **chat:** purpose を指定してタスクを投入できるようにする ([#643](https://github.com/fohte/t-rader/issues/643)) ([44fbab7](https://github.com/fohte/t-rader/commit/44fbab79033bafc7977b36f5b9a11560be9a8b51))
* **terraform-provider:** agent_config リソースを追加する ([#632](https://github.com/fohte/t-rader/issues/632)) ([6755793](https://github.com/fohte/t-rader/commit/675579332d3bb71e6ca921d5558bbef2e6231384))
* **terraform-provider:** RSS フィードリソースを追加する ([#639](https://github.com/fohte/t-rader/issues/639)) ([c50c8dd](https://github.com/fohte/t-rader/commit/c50c8dde52ced0c4bf0c4ccded003e3136c5772d))
* **terraform-provider:** trader_custom_indicator リソースを追加する ([#634](https://github.com/fohte/t-rader/issues/634)) ([b28716b](https://github.com/fohte/t-rader/commit/b28716b03320e015529f428aacb6a55af2fff806))
* **terraform-provider:** trader_risk_limit リソースを追加 ([#627](https://github.com/fohte/t-rader/issues/627)) ([6cc6a60](https://github.com/fohte/t-rader/commit/6cc6a6029ef6361cc916d3f5576f19e482cf74d9))
* **terraform-provider:** 戦略 trigger リソースを追加 ([#631](https://github.com/fohte/t-rader/issues/631)) ([91c83ae](https://github.com/fohte/t-rader/commit/91c83aeac7da34ab1788f64f1012781dc3dbbc33))
* **trigger:** purpose を指定可能にする ([#654](https://github.com/fohte/t-rader/issues/654)) ([bd334c8](https://github.com/fohte/t-rader/commit/bd334c82a45fae51209cd13634d05dc75446edd7))


### Miscellaneous Chores

* **terraform-provider:** trigger release ([2107276](https://github.com/fohte/t-rader/commit/2107276b03b2dad79815a231eb549ebc706b5065))
