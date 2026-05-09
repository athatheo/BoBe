prompt-agent-job-evaluation-system = コーディングエージェントがタスクを完了できたか評価して。ユーザーが依頼し、エージェントが結果を出して終了した。結果の要約をもとに、目標を達成できたか判断すること。
prompt-agent-job-evaluation-original-task = 元のタスク: { $user_intent }
prompt-agent-job-evaluation-agent-result = エージェント結果: { $result_summary }
prompt-agent-job-evaluation-no-summary = 要約はありません。
prompt-agent-job-evaluation-agent-error = エージェントエラー: { $error }
prompt-agent-job-evaluation-continuation-count = このエージェントはすでに { $count } 回継続されています。
prompt-agent-job-evaluation-final-directive = 元のタスクは達成できた？ 回答は DONE か CONTINUE の1語で。完了済み、またはエージェントでは解決できないエラー（依存関係不足、プロジェクト違いなど）なら DONE。途中まで進んでいて、もう一度やれば終わりそうなら CONTINUE。
