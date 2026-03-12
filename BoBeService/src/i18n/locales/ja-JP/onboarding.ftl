onboarding-local-tier-small-label = 小 (4B)
onboarding-local-tier-small-description = 高速で省リソース。短いやり取りに適しています。
onboarding-local-tier-medium-label = 中 (8B)
onboarding-local-tier-medium-description = 性能と品質のバランスが取れています。
onboarding-local-tier-large-label = 大 (14B)
onboarding-local-tier-large-description = 最も高品質ですが、より多くのリソースが必要です。

onboarding-cloud-provider-openai-label = OpenAI
onboarding-cloud-provider-azure-openai-label = Azure OpenAI
onboarding-cloud-provider-openai-model-gpt-5-mini-label = GPT-5 Mini
onboarding-cloud-provider-openai-model-gpt-5-nano-label = GPT-5 Nano
onboarding-cloud-provider-openai-model-gpt-5-2-label = GPT-5.2

setup-label-validate = システムを検証
setup-label-engine = エンジンを起動
setup-label-text-model = テキストモデルをダウンロード
setup-label-embedding-model = エンベディングモデルをダウンロード
setup-label-embedding-warmup = エンベディングをテスト
setup-label-vision-model = ビジョンモデルをダウンロード
setup-label-persist = 設定を保存

setup-step-validating = システムを検証中…
setup-step-engine-starting = Ollamaを起動中…
setup-step-persisting = 設定を保存中…

setup-error-create-data-directory = データディレクトリを作成できません: { $error }
setup-error-not-enough-disk-space = ディスク容量が不足しています: 約{ $needed_gb } GB 必要、空き { $available_gb } GB
setup-error-unknown-provider = 不明なプロバイダー: { $provider }
setup-error-unknown-mode = 不明なモード: { $mode }
setup-error-job-not-found = セットアップジョブ '{ $job_id }' が見つかりません
setup-error-persist-failed = 設定を保存できませんでした

setup-step-validate-data-directory-ready = データディレクトリの準備完了
setup-step-engine-ollama-at = Ollama: { $path }
setup-step-model-pulling = { $model } をプル中
setup-step-model-ready = { $model } の準備完了
setup-step-vision-model-pull-failed-non-fatal = ビジョンモデルのプルに失敗しました（続行に影響なし）: { $error }
setup-step-embedding-loading = エンベディングモデルをメモリに読み込み中...
setup-step-embedding-loaded = エンベディングモデルを読み込みました
setup-step-embedding-warmup-failed-non-fatal = ウォームアップに失敗しました（続行に影響なし）: { $error }
setup-step-persist-saved = 設定を保存しました

setup-openai-error-api-key-required = OpenAIにはAPIキーが必要です
setup-openai-validation-api-key-valid = APIキーは有効です
setup-openai-error-validation-http = APIキーの検証に失敗しました: HTTP { $status }
setup-openai-error-invalid-api-key-format = OpenAI APIキーの形式が正しくありません。空白や改行を削除して、もう一度お試しください。
setup-openai-error-cannot-reach = OpenAIに接続できません: { $error }
setup-openai-embedding-testing = エンベディングエンドポイントをテスト中...
setup-openai-embedding-working = エンベディングエンドポイントは正常です
setup-openai-embedding-failed = エンベディングテストに失敗しました: { $error }

setup-azure-error-api-key-required = APIキーが必要です
setup-azure-error-endpoint-required = エンドポイントが必要です
setup-azure-error-deployment-required = デプロイが必要です
setup-azure-validation-endpoint-validated = Azureエンドポイントを確認しました
setup-azure-error-validation-http = Azureの検証に失敗しました: HTTP { $status }
setup-azure-error-invalid-value-format = Azure設定値の形式が正しくありません。空白や改行を削除して、もう一度お試しください。
setup-azure-error-cannot-reach = Azureエンドポイントに接続できません: { $error }
setup-azure-embedding-testing = エンベディングをテスト中...
setup-azure-embedding-working = エンベディングは正常です
setup-azure-embedding-failed = エンベディングに失敗しました: { $error }
