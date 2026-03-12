onboarding-local-tier-small-label = Pequeño (4B)
onboarding-local-tier-small-description = Rápido y ligero. Ideal para consultas rápidas.
onboarding-local-tier-medium-label = Mediano (8B)
onboarding-local-tier-medium-description = Equilibrio entre rendimiento y calidad.
onboarding-local-tier-large-label = Grande (14B)
onboarding-local-tier-large-description = La mejor calidad, requiere más recursos.

onboarding-cloud-provider-openai-label = OpenAI
onboarding-cloud-provider-azure-openai-label = Azure OpenAI
onboarding-cloud-provider-openai-model-gpt-5-mini-label = GPT-5 Mini
onboarding-cloud-provider-openai-model-gpt-5-nano-label = GPT-5 Nano
onboarding-cloud-provider-openai-model-gpt-5-2-label = GPT-5.2

setup-label-validate = Validar sistema
setup-label-engine = Iniciar motor
setup-label-text-model = Descargar modelo de texto
setup-label-embedding-model = Descargar modelo de incrustaciones
setup-label-embedding-warmup = Probar incrustaciones
setup-label-vision-model = Descargar modelo de visión
setup-label-persist = Guardar configuración

setup-step-validating = Validando el sistema…
setup-step-engine-starting = Iniciando Ollama…
setup-step-persisting = Guardando configuración…

setup-error-create-data-directory = No se puede crear el directorio de datos: { $error }
setup-error-not-enough-disk-space = Espacio en disco insuficiente: se requieren ~{ $needed_gb } GB, disponibles { $available_gb } GB
setup-error-unknown-provider = Proveedor desconocido: { $provider }
setup-error-unknown-mode = Modo desconocido: { $mode }
setup-error-job-not-found = No se encontró el trabajo de configuración '{ $job_id }'
setup-error-persist-failed = No se pudo guardar la configuración

setup-step-validate-data-directory-ready = Directorio de datos listo
setup-step-engine-ollama-at = Ollama en { $path }
setup-step-model-pulling = Descargando { $model }
setup-step-model-ready = { $model } listo
setup-step-vision-model-pull-failed-non-fatal = Falló la descarga del modelo de visión (no fatal): { $error }
setup-step-embedding-loading = Cargando el modelo de incrustaciones en memoria...
setup-step-embedding-loaded = Modelo de incrustaciones cargado
setup-step-embedding-warmup-failed-non-fatal = Falló el precalentamiento (no crítico): { $error }
setup-step-persist-saved = Configuración guardada

setup-openai-error-api-key-required = La clave de API es obligatoria para OpenAI
setup-openai-validation-api-key-valid = Clave de API válida
setup-openai-error-validation-http = La validación de la clave de API falló: HTTP { $status }
setup-openai-error-invalid-api-key-format = Formato de clave de API de OpenAI no válido. Elimina espacios/saltos de línea y vuelve a intentarlo.
setup-openai-error-cannot-reach = No se puede conectar con OpenAI: { $error }
setup-openai-embedding-testing = Probando el punto de conexión de incrustaciones...
setup-openai-embedding-working = El punto de conexión de incrustaciones funciona
setup-openai-embedding-failed = Falló la prueba de incrustaciones: { $error }

setup-azure-error-api-key-required = La clave de API es obligatoria
setup-azure-error-endpoint-required = El punto de conexión es obligatorio
setup-azure-error-deployment-required = La implementación es obligatoria
setup-azure-validation-endpoint-validated = Punto de conexión de Azure validado
setup-azure-error-validation-http = La validación de Azure falló: HTTP { $status }
setup-azure-error-invalid-value-format = Formato de valor de configuración de Azure no válido. Elimina espacios/saltos de línea y vuelve a intentarlo.
setup-azure-error-cannot-reach = No se puede conectar con el punto de conexión de Azure: { $error }
setup-azure-embedding-testing = Probando incrustaciones...
setup-azure-embedding-working = Las incrustaciones funcionan
setup-azure-embedding-failed = Fallaron las incrustaciones: { $error }
