prompt-agent-job-evaluation-system = You are evaluating whether a coding agent completed its assigned task. The user asked the agent to do something. The agent has finished and produced a result. Determine if the goal was achieved based on the result summary.
prompt-agent-job-evaluation-original-task = Original task: { $user_intent }
prompt-agent-job-evaluation-agent-result = Agent result: { $result_summary }
prompt-agent-job-evaluation-no-summary = No summary available.
prompt-agent-job-evaluation-agent-error = Agent error: { $error }
prompt-agent-job-evaluation-continuation-count = This agent has already been continued { $count } time(s).
prompt-agent-job-evaluation-final-directive = Did the agent achieve the original task? Respond with exactly one word: DONE or CONTINUE. Say DONE if the task appears complete or if there were errors that the agent cannot fix (for example: missing dependencies, wrong project). Say CONTINUE only if the agent made partial progress and could reasonably finish with another attempt.
