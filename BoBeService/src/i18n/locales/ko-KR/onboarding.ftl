onboarding-local-tier-small-label = 소형 (4B)
onboarding-local-tier-small-description = 빠르고 리소스를 적게 써요. 간단한 대화에 적합해요.
onboarding-local-tier-medium-label = 중형 (8B)
onboarding-local-tier-medium-description = 성능과 품질의 균형이 좋아요.
onboarding-local-tier-large-label = 대형 (14B)
onboarding-local-tier-large-description = 가장 높은 품질이지만 리소스가 더 필요해요.

onboarding-cloud-provider-openai-label = OpenAI
onboarding-cloud-provider-azure-openai-label = Azure OpenAI
onboarding-cloud-provider-openai-model-gpt-5-mini-label = GPT-5 Mini
onboarding-cloud-provider-openai-model-gpt-5-nano-label = GPT-5 Nano
onboarding-cloud-provider-openai-model-gpt-5-2-label = GPT-5.2

setup-label-validate = 시스템 확인
setup-label-engine = 엔진 시작
setup-label-text-model = 텍스트 모델 다운로드
setup-label-embedding-model = 임베딩 모델 다운로드
setup-label-embedding-warmup = 임베딩 테스트
setup-label-vision-model = 비전 모델 다운로드
setup-label-persist = 설정 저장

setup-step-validating = 시스템 확인 중…
setup-step-engine-starting = Ollama 시작 중…
setup-step-persisting = 설정 저장 중…

setup-error-create-data-directory = 데이터 디렉터리를 만들 수 없어요: { $error }
setup-error-not-enough-disk-space = 디스크 공간이 부족해요: 약 { $needed_gb } GB 필요, { $available_gb } GB 사용 가능
setup-error-unknown-provider = 알 수 없는 제공자: { $provider }
setup-error-unknown-mode = 알 수 없는 모드: { $mode }
setup-error-job-not-found = 설정 작업 '{ $job_id }'을(를) 찾을 수 없어요
setup-error-persist-failed = 설정을 저장하지 못했어요

setup-step-validate-data-directory-ready = 데이터 디렉터리 준비 완료
setup-step-engine-ollama-at = Ollama 위치: { $path }
setup-step-model-pulling = { $model } 가져오는 중
setup-step-model-ready = { $model } 준비 완료
setup-step-vision-model-pull-failed-non-fatal = 비전 모델 다운로드 실패(계속 진행 가능): { $error }
setup-step-embedding-loading = 임베딩 모델을 메모리에 불러오는 중...
setup-step-embedding-loaded = 임베딩 모델 불러오기 완료
setup-step-embedding-warmup-failed-non-fatal = 워밍업 실패(계속 진행 가능): { $error }
setup-step-persist-saved = 설정 저장 완료

setup-openai-error-api-key-required = OpenAI를 사용하려면 API 키가 필요해요
setup-openai-validation-api-key-valid = API 키 확인 완료
setup-openai-error-validation-http = API 키 확인 실패: HTTP { $status }
setup-openai-error-invalid-api-key-format = OpenAI API 키 형식이 올바르지 않아요. 공백이나 줄바꿈을 제거하고 다시 시도해 주세요.
setup-openai-error-cannot-reach = OpenAI에 연결할 수 없어요: { $error }
setup-openai-embedding-testing = 임베딩 엔드포인트 테스트 중...
setup-openai-embedding-working = 임베딩 엔드포인트 정상 동작
setup-openai-embedding-failed = 임베딩 테스트 실패: { $error }

setup-azure-error-api-key-required = API 키가 필요해요
setup-azure-error-endpoint-required = 엔드포인트가 필요해요
setup-azure-error-deployment-required = 배포 이름이 필요해요
setup-azure-validation-endpoint-validated = Azure 엔드포인트 확인 완료
setup-azure-error-validation-http = Azure 확인 실패: HTTP { $status }
setup-azure-error-invalid-value-format = Azure 설정 값 형식이 올바르지 않아요. 공백이나 줄바꿈을 제거하고 다시 시도해 주세요.
setup-azure-error-cannot-reach = Azure 엔드포인트에 연결할 수 없어요: { $error }
setup-azure-embedding-testing = 임베딩 테스트 중...
setup-azure-embedding-working = 임베딩 정상 동작
setup-azure-embedding-failed = 임베딩 실패: { $error }
