onboarding-local-tier-small-label = 小型（4B）
onboarding-local-tier-small-description = 速度快、资源占用低，适合快速互动。
onboarding-local-tier-medium-label = 中型（8B）
onboarding-local-tier-medium-description = 兼顾性能和质量。
onboarding-local-tier-large-label = 大型（14B）
onboarding-local-tier-large-description = 质量最佳，但需要更多资源。

onboarding-cloud-provider-openai-label = OpenAI
onboarding-cloud-provider-azure-openai-label = Azure OpenAI
onboarding-cloud-provider-openai-model-gpt-5-mini-label = GPT-5 Mini
onboarding-cloud-provider-openai-model-gpt-5-nano-label = GPT-5 Nano
onboarding-cloud-provider-openai-model-gpt-5-2-label = GPT-5.2

setup-label-validate = 验证系统
setup-label-engine = 启动引擎
setup-label-text-model = 下载文本模型
setup-label-embedding-model = 下载嵌入模型
setup-label-embedding-warmup = 测试嵌入
setup-label-vision-model = 下载视觉模型
setup-label-persist = 保存配置

setup-step-validating = 正在验证系统…
setup-step-engine-starting = 正在启动 Ollama…
setup-step-persisting = 正在保存配置…

setup-error-create-data-directory = 无法创建数据目录：{ $error }
setup-error-not-enough-disk-space = 磁盘空间不足：需要 ~{ $needed_gb } GB，可用 { $available_gb } GB
setup-error-unknown-provider = 未知提供商：{ $provider }
setup-error-unknown-mode = 未知模式：{ $mode }
setup-error-job-not-found = 未找到设置任务“{ $job_id }”
setup-error-persist-failed = 保存配置失败

setup-step-validate-data-directory-ready = 数据目录已就绪
setup-step-engine-ollama-at = Ollama 位于 { $path }
setup-step-model-pulling = 正在下载 { $model }
setup-step-model-ready = { $model } 已就绪
setup-step-vision-model-pull-failed-non-fatal = 视觉模型下载失败（非致命）：{ $error }
setup-step-embedding-loading = 正在将嵌入模型加载到内存中...
setup-step-embedding-loaded = 嵌入模型已加载
setup-step-embedding-warmup-failed-non-fatal = 预热失败（非致命）：{ $error }
setup-step-persist-saved = 配置已保存

setup-openai-error-api-key-required = 请填写 OpenAI API 密钥
setup-openai-validation-api-key-valid = API 密钥有效
setup-openai-error-validation-http = API 密钥验证失败：HTTP { $status }
setup-openai-error-invalid-api-key-format = API 密钥格式不正确，请去掉多余的空格或换行后重试。
setup-openai-error-cannot-reach = 无法连接 OpenAI：{ $error }
setup-openai-embedding-testing = 正在测试嵌入端点...
setup-openai-embedding-working = 嵌入端点可用
setup-openai-embedding-failed = 嵌入测试失败：{ $error }

setup-azure-error-api-key-required = 请填写 API 密钥
setup-azure-error-endpoint-required = 请填写终结点
setup-azure-error-deployment-required = 请填写部署名称
setup-azure-validation-endpoint-validated = Azure 终结点已通过验证
setup-azure-error-validation-http = Azure 验证失败：HTTP { $status }
setup-azure-error-invalid-value-format = 格式不正确，请去掉多余的空格或换行后重试。
setup-azure-error-cannot-reach = 无法连接 Azure 终结点：{ $error }
setup-azure-embedding-testing = 正在测试嵌入...
setup-azure-embedding-working = 嵌入可用
setup-azure-embedding-failed = 嵌入失败：{ $error }
