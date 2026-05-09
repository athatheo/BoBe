prompt-agent-job-evaluation-system = 코딩 에이전트가 맡은 작업을 완료했는지 평가하는 거야. 사용자가 에이전트에게 뭔가를 시켰고, 에이전트가 결과를 냈어. 결과 요약을 보고 목표를 달성했는지 판단해 줘.
prompt-agent-job-evaluation-original-task = 원래 작업: { $user_intent }
prompt-agent-job-evaluation-agent-result = 에이전트 결과: { $result_summary }
prompt-agent-job-evaluation-no-summary = 요약 없음.
prompt-agent-job-evaluation-agent-error = 에이전트 오류: { $error }
prompt-agent-job-evaluation-continuation-count = 이 에이전트는 이미 { $count }번 재시도했어.
prompt-agent-job-evaluation-final-directive = 에이전트가 원래 작업을 달성했어? 정확히 한 단어로만 답해: DONE 또는 CONTINUE. 작업이 완료된 것 같거나 에이전트가 해결할 수 없는 오류(예: 의존성 누락, 잘못된 프로젝트)면 DONE. 부분적으로 진행됐고 한 번 더 하면 끝낼 수 있을 것 같을 때만 CONTINUE.
