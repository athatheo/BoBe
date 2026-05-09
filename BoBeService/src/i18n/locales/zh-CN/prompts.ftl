prompt-agent-job-evaluation-system = 你在评估一个 coding agent 有没有完成分配的任务。用户给 agent 提了个要求，agent 跑完了出了结果。根据结果摘要判断一下目标有没有达成。
prompt-agent-job-evaluation-original-task = 原始任务：{ $user_intent }
prompt-agent-job-evaluation-agent-result = Agent 结果：{ $result_summary }
prompt-agent-job-evaluation-no-summary = 没有可用摘要。
prompt-agent-job-evaluation-agent-error = Agent 错误：{ $error }
prompt-agent-job-evaluation-continuation-count = 这个 agent 已经被续跑了 { $count } 次。
prompt-agent-job-evaluation-final-directive = Agent 完成原始任务了吗？只回一个词：DONE 或 CONTINUE。如果任务看起来搞定了，或者碰到 agent 自己修不了的错误（比如缺依赖、项目有问题），就回 DONE。只有 agent 已经有进展、再试一次有可能搞定的情况下，才回 CONTINUE。
