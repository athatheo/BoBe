onboarding-local-tier-small-label = 소형 (4B)
onboarding-local-tier-small-description = 빠르고 리소스 사용량이 낮습니다. 빠른 상호작용에 적합합니다.
onboarding-local-tier-medium-label = 중형 (8B)
onboarding-local-tier-medium-description = 성능과 품질의 균형이 좋습니다.
onboarding-local-tier-large-label = 대형 (14B)
onboarding-local-tier-large-description = 가장 높은 품질을 제공하지만 더 많은 리소스가 필요합니다.

onboarding-cloud-provider-openai-label = OpenAI
onboarding-cloud-provider-azure-openai-label = Azure OpenAI
onboarding-cloud-provider-openai-model-gpt-5-mini-label = GPT-5 Mini
onboarding-cloud-provider-openai-model-gpt-5-nano-label = GPT-5 Nano
onboarding-cloud-provider-openai-model-gpt-5-2-label = GPT-5.2

setup-error-create-data-directory = 데이터 디렉터리를 생성할 수 없습니다: { $error }
setup-error-not-enough-disk-space = 디스크 공간이 부족합니다: 약 { $needed_gb } GB 필요, 사용 가능 { $available_gb } GB
setup-error-unknown-provider = 알 수 없는 제공자: { $provider }
setup-error-unknown-mode = 알 수 없는 모드: { $mode }
setup-error-job-not-found = 설정 작업 '{ $job_id }'을(를) 찾을 수 없습니다
setup-error-persist-failed = 설정을 저장하지 못했습니다

setup-step-validate-data-directory-ready = 데이터 디렉터리 준비 완료
setup-step-engine-ollama-at = Ollama 위치: { $path }
setup-step-model-pulling = { $model } 가져오는 중
setup-step-model-ready = { $model } 준비 완료
setup-step-vision-model-pull-failed-non-fatal = 비전 모델 가져오기 실패(치명적이지 않음): { $error }
setup-step-embedding-loading = 임베딩 모델을 메모리에 로드하는 중...
setup-step-embedding-loaded = 임베딩 모델 로드 완료
setup-step-embedding-warmup-failed-non-fatal = 워밍업 실패(치명적이지 않음): { $error }
setup-step-persist-saved = 설정 저장 완료

setup-openai-error-api-key-required = OpenAI에는 API 키가 필요합니다
setup-openai-validation-api-key-valid = API 키가 유효합니다
setup-openai-error-validation-http = API 키 검증 실패: HTTP { $status }
setup-openai-error-invalid-api-key-format = OpenAI API 키 형식이 잘못되었습니다. 공백/줄바꿈을 제거하고 다시 시도하세요.
setup-openai-error-cannot-reach = OpenAI에 연결할 수 없습니다: { $error }
setup-openai-embedding-testing = 임베딩 엔드포인트 테스트 중...
setup-openai-embedding-working = 임베딩 엔드포인트 정상 동작
setup-openai-embedding-failed = 임베딩 테스트 실패: { $error }

setup-azure-error-api-key-required = API 키가 필요합니다
setup-azure-error-endpoint-required = 엔드포인트가 필요합니다
setup-azure-error-deployment-required = 배포가 필요합니다
setup-azure-validation-endpoint-validated = Azure 엔드포인트 검증 완료
setup-azure-error-validation-http = Azure 검증 실패: HTTP { $status }
setup-azure-error-invalid-value-format = Azure 설정 값 형식이 잘못되었습니다. 공백/줄바꿈을 제거하고 다시 시도하세요.
setup-azure-error-cannot-reach = Azure 엔드포인트에 연결할 수 없습니다: { $error }
setup-azure-embedding-testing = 임베딩 테스트 중...
setup-azure-embedding-working = 임베딩 정상 동작
setup-azure-embedding-failed = 임베딩 실패: { $error }
