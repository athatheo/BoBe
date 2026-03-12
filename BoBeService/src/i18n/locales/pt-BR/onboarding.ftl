onboarding-local-tier-small-label = Pequeno (4B)
onboarding-local-tier-small-description = Rápido e leve. Ótimo pra interações rápidas.
onboarding-local-tier-medium-label = Médio (8B)
onboarding-local-tier-medium-description = Equilíbrio entre desempenho e qualidade.
onboarding-local-tier-large-label = Grande (14B)
onboarding-local-tier-large-description = Melhor qualidade, mas exige mais recursos.

onboarding-cloud-provider-openai-label = OpenAI
onboarding-cloud-provider-azure-openai-label = Azure OpenAI
onboarding-cloud-provider-openai-model-gpt-5-mini-label = GPT-5 Mini
onboarding-cloud-provider-openai-model-gpt-5-nano-label = GPT-5 Nano
onboarding-cloud-provider-openai-model-gpt-5-2-label = GPT-5.2

setup-label-validate = Validar sistema
setup-label-engine = Iniciar motor
setup-label-text-model = Baixar modelo de texto
setup-label-embedding-model = Baixar modelo de embeddings
setup-label-embedding-warmup = Testar embeddings
setup-label-vision-model = Baixar modelo de visão
setup-label-persist = Salvar configuração

setup-step-validating = Validando o sistema…
setup-step-engine-starting = Iniciando o Ollama…
setup-step-persisting = Salvando configuração…

setup-error-create-data-directory = Não foi possível criar o diretório de dados: { $error }
setup-error-not-enough-disk-space = Espaço em disco insuficiente: ~{ $needed_gb } GB necessários, { $available_gb } GB disponíveis
setup-error-unknown-provider = Provedor desconhecido: { $provider }
setup-error-unknown-mode = Modo desconhecido: { $mode }
setup-error-job-not-found = Tarefa de configuração '{ $job_id }' não encontrada
setup-error-persist-failed = Falha ao salvar a configuração

setup-step-validate-data-directory-ready = Diretório de dados pronto
setup-step-engine-ollama-at = Ollama em { $path }
setup-step-model-pulling = Baixando { $model }
setup-step-model-ready = { $model } pronto
setup-step-vision-model-pull-failed-non-fatal = Falha ao baixar o modelo de visão (não fatal): { $error }
setup-step-embedding-loading = Carregando modelo de embeddings na memória...
setup-step-embedding-loaded = Modelo de embeddings carregado
setup-step-embedding-warmup-failed-non-fatal = Falha no warmup (não fatal): { $error }
setup-step-persist-saved = Configuração salva

setup-openai-error-api-key-required = A chave de API é obrigatória para OpenAI
setup-openai-validation-api-key-valid = Chave de API válida
setup-openai-error-validation-http = Falha na validação da chave de API: HTTP { $status }
setup-openai-error-invalid-api-key-format = Formato de chave de API da OpenAI inválido. Remova espaços/quebras de linha e tente novamente.
setup-openai-error-cannot-reach = Não foi possível acessar a OpenAI: { $error }
setup-openai-embedding-testing = Testando endpoint de embeddings...
setup-openai-embedding-working = Endpoint de embeddings funcionando
setup-openai-embedding-failed = Falha no teste de embeddings: { $error }

setup-azure-error-api-key-required = Chave de API obrigatória
setup-azure-error-endpoint-required = Endpoint obrigatório
setup-azure-error-deployment-required = Implantação obrigatória
setup-azure-validation-endpoint-validated = Endpoint do Azure validado
setup-azure-error-validation-http = Falha na validação do Azure: HTTP { $status }
setup-azure-error-invalid-value-format = Formato de valor de configuração do Azure inválido. Remova espaços/quebras de linha e tente novamente.
setup-azure-error-cannot-reach = Não foi possível acessar o endpoint do Azure: { $error }
setup-azure-embedding-testing = Testando embeddings...
setup-azure-embedding-working = Embeddings funcionando
setup-azure-embedding-failed = Falha em embeddings: { $error }
