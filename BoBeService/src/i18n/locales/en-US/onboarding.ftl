onboarding-local-tier-small-label = Small (4B)
onboarding-local-tier-small-description = Fast, low resource usage. Good for quick interactions.
onboarding-local-tier-medium-label = Medium (8B)
onboarding-local-tier-medium-description = Balanced performance and quality.
onboarding-local-tier-large-label = Large (14B)
onboarding-local-tier-large-description = Best quality, requires more resources.

onboarding-cloud-provider-openai-label = OpenAI
onboarding-cloud-provider-azure-openai-label = Azure OpenAI
onboarding-cloud-provider-openai-model-gpt-5-mini-label = GPT-5 Mini
onboarding-cloud-provider-openai-model-gpt-5-nano-label = GPT-5 Nano
onboarding-cloud-provider-openai-model-gpt-5-2-label = GPT-5.2

setup-label-validate = Validate system
setup-label-engine = Start engine
setup-label-text-model = Download text model
setup-label-embedding-model = Download embedding model
setup-label-embedding-warmup = Test embeddings
setup-label-vision-model = Download vision model
setup-label-persist = Save configuration

setup-step-validating = Validating system…
setup-step-engine-starting = Starting Ollama…
setup-step-persisting = Saving configuration…

setup-error-create-data-directory = Cannot create data directory: { $error }
setup-error-not-enough-disk-space = Not enough disk space: ~{ $needed_gb } GB required, { $available_gb } GB available
setup-error-unknown-provider = Unknown provider: { $provider }
setup-error-unknown-mode = Unknown mode: { $mode }
setup-error-job-not-found = Setup job '{ $job_id }' not found
setup-error-persist-failed = Failed to persist configuration

setup-step-validate-data-directory-ready = Data directory ready
setup-step-engine-ollama-at = Ollama at { $path }
setup-step-model-pulling = Pulling { $model }
setup-step-model-ready = { $model } ready
setup-step-vision-model-pull-failed-non-fatal = Vision model pull failed (non-fatal): { $error }
setup-step-embedding-loading = Loading embedding model into memory...
setup-step-embedding-loaded = Embedding model loaded
setup-step-embedding-warmup-failed-non-fatal = Warmup failed (non-fatal): { $error }
setup-step-persist-saved = Configuration saved

setup-openai-error-api-key-required = API key is required for OpenAI
setup-openai-validation-api-key-valid = API key valid
setup-openai-error-validation-http = API key validation failed: HTTP { $status }
setup-openai-error-invalid-api-key-format = Invalid OpenAI API key format. Remove spaces/newlines and try again.
setup-openai-error-cannot-reach = Cannot reach OpenAI: { $error }
setup-openai-embedding-testing = Testing embedding endpoint...
setup-openai-embedding-working = Embedding endpoint working
setup-openai-embedding-failed = Embedding test failed: { $error }

setup-azure-error-api-key-required = API key required
setup-azure-error-endpoint-required = Endpoint required
setup-azure-error-deployment-required = Deployment required
setup-azure-validation-endpoint-validated = Azure endpoint validated
setup-azure-error-validation-http = Azure validation failed: HTTP { $status }
setup-azure-error-invalid-value-format = Invalid Azure setup value format. Remove spaces/newlines and try again.
setup-azure-error-cannot-reach = Cannot reach Azure endpoint: { $error }
setup-azure-embedding-testing = Testing embedding...
setup-azure-embedding-working = Embedding working
setup-azure-embedding-failed = Embedding failed: { $error }
