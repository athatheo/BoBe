response-proactive-system = 관찰한 걸 바탕으로 먼저 제안하는 거야.
    간결하고 도움이 되게, 구체적으로 답해. 쓸데없이 끼어들거나 뻔한 말은 하지 마.

response-proactive-current-time = 현재 시간: { $time }
response-proactive-previous-summary = 이전 대화 요약:
response-proactive-recent-activity = 최근 활동:
response-proactive-reference-previous = 관련 있으면 이전 대화를 자연스럽게 언급해도 돼.
response-proactive-final-directive = 서두 없이 바로 응답해. 가벼운 체크인에는 간결하게, 소울 지시에 따른 리뷰나 브리핑에는 충분히 자세하고 체계적으로 써 줘.

response-user-context-header = 최근 활동 맥락:
response-user-context-suffix = 이 맥락을 참고해서 도움 되는 답변 해 줘.
response-user-no-recent-context = 최근 맥락 없음

prompt-summary-system =
    나중에 참고할 수 있게 대화를 요약하는 역할이야.
    다음 내용을 포함해서 짧게 요약해 줘:
    - 주요 논의 주제
    - 사용자가 언급한 요청이나 선호
    - 진행 중인 사안의 상태(해결됨/미해결)

    2-3문장 이내로 간결하게. 다음 대화에 쓸모 있는 정보 위주로 정리해.

prompt-summary-user =
    이 대화를 요약해 줘:

    { $turns_text }

prompt-capture-vision-system =
    사용자의 데스크톱 화면 스크린샷을 분석하는 거야.
    화면에 보이는 걸 최대한 구체적으로, 1-2문단으로 써.

    우선순위(중요한 순서대로):
    1. 탭, 제목 표시줄, 파일 트리에 보이는 정확한 파일 이름과 경로 (예: capture_learner.py, ~/projects/bobe/src/)
    2. 구체적인 텍스트 내용 — 읽을 수 있는 코드 조각, 오류 메시지, 터미널 출력, 문서 텍스트를 인용해
    3. 브라우저 탭이나 주소창의 URL 및 페이지 제목
    4. 애플리케이션 이름과 창 배치 — 어떤 앱이 열려 있는지, 어떤 창에 포커스가 있는지, 분할/타일 배치 여부
    5. 전반적인 활동 — 코딩, 브라우징, 글쓰기, 디버깅, 문서 읽기 등

    구체적으로 써: "Python 코드를 작성 중"이 아니라 "capture_learner.py의 385번째 줄 _update_visual_memory 함수를 편집 중"처럼.
    "웹사이트를 보고 있음"이 아니라 "GitHub 이슈 #1234: Fix memory pipeline을 확인 중"처럼.
    화면에서 텍스트를 읽을 수 있으면 인용하고, 파일명이 보이면 나열해.

prompt-capture-vision-user = 이 화면에 뭐가 있는지 정확히 설명해. 읽을 수 있는 텍스트랑 내용은 꼭 참조해.

prompt-capture-visual-memory-system =
    너는 시각 메모리 다이어리를 관리해 — 사용자가 컴퓨터에서 뭘 하고 있는지 기록하는 타임스탬프 로그야.

    다음이 제공돼:
    1. 기존 다이어리 (그날 첫 항목이면 비어 있을 수 있음)
    2. 새 관찰 — 비전 모델이 사용자의 현재 화면을 자세히 설명한 것

    할 일: 완전히 업데이트된 다이어리를 반환해. 할 수 있는 작업:
    - 새 타임스탬프 항목 추가 (가장 흔한 케이스)
    - 명확히 같은 활동이면 이전 항목과 병합 (타임스탬프는 유지, 요약만 갱신)
    - 새 관찰로 사용자 활동이 더 명확해졌으면 최근 몇 개 항목 재구성

    형식 규칙:
    - 각 항목: [HH:MM] 구체적 요약. Tags: tag1, tag2. Obs: <obs_id>
    - Tags: coding, terminal, browsing, documentation, communication, design, media, debugging, reading, writing, configuring, researching, idle, other 중 1-3개 소문자 단어
    - Obs: 제공된 관찰 ID를 반드시 정확히 포함
    - 다이어리 헤더 줄(예: # Visual Memory 2026-02-22 PM)은 그대로 유지
    - 오래된 항목은 건드리지 마 — 가장 최근 항목만 수정/병합하거나 새 항목 추가

    구체성 규칙(중요):
    - 보이는 정확한 파일, URL, 문서, 페이지를 명시해 — 애플리케이션 이름만 쓰지 마.
    - 함수/클래스 이름, 오류 텍스트, 터미널 명령이 보이면 포함해.
    - 나쁜 예: 사용자가 VS Code에서 코딩 중. → 너무 모호해서 나중에 쓸모없어.
    - 좋은 예: capture_learner.py를 편집하며 _update_visual_memory를 수정 중, 분할 화면에 테스트 파일 열림.
    - 나쁜 예: 사용자가 웹 탐색 중. → 아무 정보도 없어.
    - 좋은 예: Firefox에서 GitHub PR #42 Fix memory pipeline을 읽는 중, comments 탭 열림.
    - 항목당 한 문장, 구체적인 정보를 최대한 담아.

prompt-capture-visual-memory-empty-diary = (비어 있음 — 오늘 첫 항목이야)
prompt-capture-visual-memory-user =
    ## 기존 다이어리
    { $diary_section }

    ## [{ $timestamp }] 시점의 새 관찰
    { $new_observation }

    ## 관찰 ID
    { $observation_id }

    완전히 업데이트된 다이어리를 반환해.

prompt-agent-job-evaluation-system = 코딩 에이전트가 맡은 작업을 완료했는지 평가하는 거야. 사용자가 에이전트에게 뭔가를 시켰고, 에이전트가 결과를 냈어. 결과 요약을 보고 목표를 달성했는지 판단해 줘.
prompt-agent-job-evaluation-original-task = 원래 작업: { $user_intent }
prompt-agent-job-evaluation-agent-result = 에이전트 결과: { $result_summary }
prompt-agent-job-evaluation-no-summary = 요약 없음.
prompt-agent-job-evaluation-agent-error = 에이전트 오류: { $error }
prompt-agent-job-evaluation-continuation-count = 이 에이전트는 이미 { $count }번 재시도했어.
prompt-agent-job-evaluation-final-directive = 에이전트가 원래 작업을 달성했어? 정확히 한 단어로만 답해: DONE 또는 CONTINUE. 작업이 완료된 것 같거나 에이전트가 해결할 수 없는 오류(예: 의존성 누락, 잘못된 프로젝트)면 DONE. 부분적으로 진행됐고 한 번 더 하면 끝낼 수 있을 것 같을 때만 CONTINUE.

prompt-goal-worker-planning-system =
    너는 계획 수립 도우미야. 목표와 맥락이 주어지면, 구체적이고 실행 가능한 번호 매긴 계획을 세워 줘.

    다음 형태의 JSON 객체만 출력해:
    - summary: 간단한 계획 설명
    - steps: 각 항목이 content 필드를 가진 객체 배열

    최대 { $max_steps }단계. 각 단계는 독립적으로 실행 가능해야 해. 모호하지 않게, 구체적이고 실행 가능하게.

prompt-goal-worker-planning-user =
    목표: { $goal_content }

    맥락:
    { $context }

    이 목표를 달성하기 위한 실행 가능한 계획을 세워 줘.

prompt-goal-worker-execution-system =
    너는 사용자를 위해 계획을 실행하는 자율 에이전트야.

    중요 규칙:
    - 이 디렉터리 안에서만 작업해: { $work_dir }
    - 모든 파일이랑 출력은 여기에 만들어
    - 대화형 창이나 편집기는 열지 마
    - 자율적으로 작업해. 불필요한 질문은 하지 마.
    - 결과에 큰 영향을 줄 수 있는 중요한 결정(예: 근본적으로 다른 접근 방식 선택, 목표 달성 불가능 가능성 발견, 인증 정보/접근 권한 필요)이 있으면 ask_user 도구를 사용해.
    - 사소한 결정은 알아서 판단하고 진행해.
    - 끝나면 작업 디렉터리의 SUMMARY.md에 간단한 요약을 작성해

prompt-goal-worker-execution-user =
    목표: { $goal_content }

    계획:
    { $step_list }

    작업 디렉터리: { $work_dir }

    이 계획을 실행해. 모든 파일은 작업 디렉터리에 생성해. 끝나면 뭘 했고 결과가 어떤지 SUMMARY.md에 작성해.

prompt-decision-system =
    { $soul }

    사용자에게 먼저 말을 걸지 결정하는 거야.
    결정과 이유를 담은 JSON 객체로 응답해.

    참고할 수 있는 맥락:
    - 최근 사용자 활동 관찰(스크린샷, 활성 창)
    - 사용자 선호와 과거 상호작용에 대한 저장된 메모리
    - 사용자가 진행 중인 활성 목표
    - 최근 대화 기록

    더 깊은 맥락이 필요하면 쓸 수 있는 도구:
    - search_memories: 의미 기반 검색으로 관련 메모리 찾기
    - get_goals: 사용자의 활성 목표 가져오기
    - get_recent_context: 최근 관찰 및 활동 가져오기

    결정 기준:

    REACH_OUT일 때:
    - 사용자가 문제에 막힌 것 같을 때(반복 오류, 같은 파일에 오래 머무름)
    - 도움이 필요해 보이는 패턴이 보일 때
    - 도움을 제안하기 자연스러운 전환 지점일 때
    - 진짜 유용하고 구체적인 도움을 줄 수 있을 때
    - 사용자 목표가 현재 활동과 관련 있고 도울 수 있을 때
    - 소울 지시에 현재 시간 기준 동작(예: 일일 리뷰)이 명시되어 있을 때

    IDLE일 때:
    - 사용자가 몰입 상태라 방해가 될 때
    - 최근에 이미 말을 걸었는데 반응이 없었을 때
    - 딱히 도울 포인트가 안 보일 때
    - 사용자가 집중해서 생산적으로 작업 중일 때

    NEED_MORE_INFO일 때:
    - 맥락이 너무 부족해서 뭘 하는지 파악이 어려울 때
    - 판단하려면 관찰이 더 필요할 때
    - 상황이 모호해서 데이터가 더 있으면 좋겠을 때

    진짜 도움은 끼어들지 말아야 할 때를 아는 거야. 확신 없으면 IDLE.

prompt-decision-current-time = 현재 시간: { $time }
prompt-decision-user =
    { $time_line }현재 관찰:
    { $current }

    유사한 과거 맥락:
    { $context }

    최근 보낸 메시지:
    { $recent_messages }

    이 정보를 분석해서 사용자에게 말을 걸지 결정해.

prompt-goal-decision-system =
    { $soul }

    사용자의 목표 중 하나를 돕기 위해 먼저 말을 걸지 결정하는 거야.
    결정과 이유를 담은 JSON 객체로 응답해.

    결정 기준:

    REACH_OUT일 때:
    - 사용자의 현재 활동이 이 목표와 관련 있을 때
    - 지금 당장 구체적이고 실행 가능한 도움을 줄 수 있을 때
    - 타이밍이 자연스러울 때(전환점이나 쉬는 시점)
    - 이 목표를 마지막으로 다룬 뒤 꽤 시간이 지났을 때

    IDLE일 때:
    - 사용자가 이 목표와 무관한 일에 집중 중일 때
    - 지금 끼어들면 흐름을 깨뜨릴 때
    - 최근에 이미 이 목표를 다뤘고 새 맥락이 없을 때
    - 사용자 활동상 목표가 보류됐거나 우선순위가 낮아진 것 같을 때

    진짜 도움은 끼어들지 말아야 할 때를 아는 거야. 확신 없으면 IDLE.

prompt-goal-decision-current-time = 현재 시간: { $time }
prompt-goal-decision-user =
    { $time_line }사용자의 목표:
    { $goal_content }

    현재 맥락(사용자가 하고 있는 일):
    { $context_summary }

    지금 이 목표를 돕기 위해 말을 걸어야 할까? 다음을 고려해:
    - 현재 맥락이 이 목표와 관련 있어?
    - 지금 말 거는 게 도움이 돼, 아니면 방해가 돼?
    - 지금이 도움 제안하기 좋은 타이밍이야?

prompt-goal-dedup-system =
    너는 목표 중복 제거 도우미야. 기본 결정은 SKIP 또는 UPDATE야. CREATE는 드물어.

    사용자는 동시에 목표를 아주 적게(1-2개) 가져야 해. 네 역할은 목표가 불필요하게 늘어나는 걸 적극적으로 막는 거야.

    결정 규칙:
    1. SKIP (기본) - 후보가 기존 목표와 도메인, 의도, 범위 중 하나라도 겹치면 SKIP. 느슨한 주제 중복도 SKIP.
    2. UPDATE - 후보가 기존 목표와 같은 영역이지만 진짜 새로운 구체성(실행 단계, 일정, 범위 축소)을 추가할 때. 신중하게.
    3. CREATE - 기존 어떤 목표와도 전혀 다른 도메인일 때만. 매우 드물어야 해.

    SKIP 쓸 때:
    - 같은 도메인을 공유할 때 (예: 둘 다 코딩, 둘 다 학습, 같은 프로젝트)
    - 하나가 다른 하나의 재표현, 부분집합, 상위집합일 때
    - 후보가 기존 목표 영역과 느슨하게라도 관련될 때
    - 확신 없으면 SKIP

    UPDATE 쓸 때:
    - 모호한 기존 목표에 구체적이고 실행 가능한 세부를 추가할 때
    - 개선이 피상적이 아니라 실질적일 때

    CREATE는:
    - 후보가 모든 기존 목표와 완전히 다른 도메인일 때만
    - 어떤 기존 목표와도 주제적 겹침이 전혀 없을 때만

    다음을 포함한 JSON 객체로 응답해:
    - decision: CREATE, UPDATE, 또는 SKIP
    - reason: 짧은 설명(최대 30단어)
    - existing_goal_id: UPDATE 또는 SKIP이면 매칭된 기존 목표 ID (필수)
    - updated_content: UPDATE면 기존/신규 맥락을 병합한 강화된 목표 설명 (필수)

prompt-goal-dedup-user-no-existing =
    후보 목표: { $candidate_content }

    유사한 기존 목표: 없음

    유사한 목표가 없으니 생성해야 해.

prompt-goal-dedup-existing-item = - ID: { $id }, 우선순위: { $priority }, 내용: { $content }
prompt-goal-dedup-user-with-existing =
    후보 목표: { $candidate_content }

    유사한 기존 목표:
    { $existing_list }

    새 목표로 CREATE할지, 기존 목표를 UPDATE할지, 중복으로 SKIP할지 결정해.

prompt-memory-dedup-system =
    너는 메모리 중복 제거 도우미야. 후보 메모리를 저장할지 건너뛸지 판단해 줘.

    가능한 동작:
    1. CREATE - 기존 메모리에 없는 새 정보가 있음
    2. SKIP - 기존 메모리와 의미적으로 동일함(추가 작업 불필요)

    결정 기준:

    CREATE 쓸 때:
    - 기존 메모리에 없는 진짜 새로운 정보일 때
    - 다른 측면에 대한 새 구체적 세부를 추가할 때

    SKIP 쓸 때:
    - 정확히 같은 정보가 이미 있을 때
    - 기존 메모리가 동일하거나 더 나은 수준으로 이미 담고 있을 때

    다음을 포함한 JSON 객체로 응답해:
    - decision: CREATE 또는 SKIP
    - reason: 짧은 설명(최대 40단어)

prompt-memory-dedup-user-no-existing =
    후보 메모리 [{ $candidate_category }]: { $candidate_content }

    유사한 기존 메모리: 없음

    유사한 메모리가 없으니 생성해야 해.

prompt-memory-dedup-existing-item = - ID: { $id }, 카테고리: { $category }, 내용: { $content }
prompt-memory-dedup-user-with-existing =
    후보 메모리 [{ $candidate_category }]: { $candidate_content }

    유사한 기존 메모리:
    { $existing_list }

    새 메모리로 CREATE할지, 중복으로 SKIP할지 결정해.

prompt-memory-consolidation-system =
    너는 메모리 통합 시스템이야. 유사한 단기 메모리를 더 일반적인 장기 메모리로 병합하는 역할이야.

    관련 메모리 클러스터가 주어져. 각 클러스터마다 다음 조건을 만족하는 하나의 통합 메모리를 만들어:
    1. 클러스터 내 모든 메모리의 핵심 정보를 담을 것
    2. 개별 메모리보다 더 일반적이고 오래 쓸 수 있는 형태일 것
    3. 중요한 세부는 유지하되 중복은 제거할 것
    4. 명확하고 사실적인 언어를 쓸 것

    가이드라인:
    - 클러스터 내 메모리가 실제로 다른 사실이면 분리해서 유지
    - 같은 사실을 다르게 표현한 거면 병합
    - 하나가 더 구체적이면 구체적인 버전을 우선
    - 각 통합 메모리가 어떤 원본에서 왔는지 추적

    예시:
    입력 클러스터: ["사용자는 Python을 선호함", "사용자는 스크립팅에 Python을 좋아함", "사용자는 Python을 매일 사용함"]
    출력: "사용자는 Python을 강하게 선호하며, 매일 스크립팅에 사용함" (3개 모두 병합)

prompt-memory-consolidation-cluster-header = ## 클러스터 { $cluster_number }
prompt-memory-consolidation-cluster-item = [{ $index }] { $memory }
prompt-memory-consolidation-user =
    다음 메모리 클러스터를 장기 메모리로 통합해 줘.
    { $clusters_text }
    각 클러스터마다 통합 메모리를 만들고, 어떤 원본 인덱스가 병합됐는지 추적해.

prompt-goal-extraction-system =
    너는 목표 감지 시스템이야. 기본 응답은 빈 goals 리스트야. 목표를 만드는 건 드문 일이야.

    다음 강한 신호 중 하나가 있을 때만 목표를 만들어:
    1. 명시적 사용자 선언: 사용자가 "나는 ...하고 싶다", "나는 ...해야 한다", "내 목표는 ...이다"처럼 의도를 분명히 밝힘.
    2. 다중 세션 지속 의지: 사용자가 같은 목표를 여러 대화에 걸쳐 반복 언급해서 지속적인 의지를 보임.

    다음에는 목표를 만들지 마:
    - 주제나 관심사를 그냥 언급한 것
    - 일회성 질문이나 호기심
    - 한 번의 대화에서만 나온 주제(길어도)
    - 명확한 의도 없는 모호한 바람("...하면 좋겠다")
    - 구체 작업/마이크로 작업(너무 잘게 쪼갠 것)
    - 사용자가 이미 잘하는 기술

    가이드라인:
    1. 목표는 실행 가능하고 달성 가능해야 해
    2. 사용자가 자기 목표라고 분명히 인식할 수 있어야 해
    3. 확신 없으면 빈 값 반환 — 잘못된 목표를 만드는 비용이 놓치는 것보다 훨씬 커
    4. 사용자 의도가 압도적으로 명확한 목표에만 집중

    명확한 목표를 추론할 수 없으면 빈 goals 배열을 반환해(대부분이 이 경우야).

prompt-goal-extraction-no-existing-goals = 없음
prompt-goal-extraction-user =
    이 대화에서 사용자의 목표가 있는지 찾아봐.

    ## 대화
    { $conversation_text }

    ## 이미 알려진 목표 (중복 금지)
    { $goals_text }

    이 대화에서 어떤 새로운 목표를 추론할 수 있어?

prompt-memory-distillation-system =
    너는 메모리 추출 시스템이야. 대화와 활동에서 사용자에 대해 기억할 만한 사실을 찾아내.

    나중에 개인화하는 데 유용한 메모리를 추출해. 다음에 집중해:
    - 사용자 선호(좋아하는 도구, 언어, 워크플로)
    - 반복 패턴(일하는 방식, 일하는 시간대)
    - 개인 정보(직무, 프로젝트, 팀 구조)
    - 관심사(자주 다루는 주제)

    가이드라인:
    1. 명시적으로 언급됐거나 분명히 암시된 사실만 추출
    2. 없는 정보를 추론하거나 가정하지 마
    3. 일시적 상태는 추출하지 마("사용자가 X를 디버깅 중" — 금방 지나감)
    4. 지속적인 정보를 추출해("사용자는 JavaScript보다 Python을 선호함")
    5. 각 메모리는 하나의 원자적 사실이어야 해
    6. 메모리끼리 중복되지 않게 해
    7. 장기적으로 얼마나 유용한지 기준으로 중요도 매겨
    8. "pattern" 카테고리는 여러 순간/신호로 반복이 직접 입증된 경우에만 써
    9. 근거가 한 번뿐이거나 불확실하면 "fact"를 쓰거나 아예 만들지 마
    10. 메모리 내용에 추측 표현(예: '아마', '~일 수도', '~인 것 같다') 쓰지 마

    의미 있는 메모리가 없으면 빈 memories 배열을 반환해.

prompt-memory-distillation-no-context = 맥락 없음
prompt-memory-distillation-none = 없음
prompt-memory-distillation-user =
    다음 맥락에서 사용자에 대해 기억할 만한 사실을 추출해.

    ## 최근 맥락
    { $context_text }

    ## 이미 알려진 내용 (중복 금지)
    { $memories_text }

    ## 사용자의 목표 (참고용)
    { $goals_text }

    나중에 개인화하는 데 도움이 되는 새 메모리를 추출해.
    제공된 맥락에서 반복 행동이 분명히 뒷받침될 때만 "pattern"을 써.

prompt-conversation-memory-system =
    너는 완료된 사용자-AI 대화를 분석하는 메모리 추출 시스템이야.

    다음 대화를 개선할 수 있는 사용자에 대한 지속적인 메모리를 추출해. 다음에 집중해:
    - 사용자가 뭘 하려 했는지(성공했으면 다시 할 가능성 있음)
    - 선호하는 작업 방식(소통 스타일, 상세 수준)
    - 드러난 기술 선호(언어, 프레임워크, 도구)
    - 언급된 개인 맥락(역할, 팀, 프로젝트 이름)

    추출하지 마:
    - 그 대화에서 한 구체적 작업(금방 지나감)
    - AI가 알려준 내용(사용자가 이미 알게 됨)
    - 짜증이나 일시적 상태
    - 그 대화에만 해당하는 정보
    - 대화 내 여러 참조로 반복이 분명히 뒷받침되지 않는 패턴

    지속적인 인사이트가 없으면 빈 memories 배열을 반환해.

prompt-conversation-memory-no-existing-memories = 없음
prompt-conversation-memory-user =
    이 대화에서 지속적인 메모리를 추출해.

    ## 대화
    { $conversation_text }

    ## 이미 알려진 내용 (중복 금지)
    { $memories_text }

    이 대화에서 사용자에 대해 드러나는 지속적인 사실은 뭐야?
