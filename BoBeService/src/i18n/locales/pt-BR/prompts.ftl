prompt-agent-job-evaluation-system = Você tá avaliando se um agente de programação concluiu a tarefa. O usuário pediu algo pro agente. O agente terminou e gerou um resultado. Avalia se o objetivo foi atingido com base no resumo.
prompt-agent-job-evaluation-original-task = Tarefa original: { $user_intent }
prompt-agent-job-evaluation-agent-result = Resultado do agente: { $result_summary }
prompt-agent-job-evaluation-no-summary = Nenhum resumo disponível.
prompt-agent-job-evaluation-agent-error = Erro do agente: { $error }
prompt-agent-job-evaluation-continuation-count = Este agente já foi continuado { $count } vez(es).
prompt-agent-job-evaluation-final-directive = O agente concluiu a tarefa original? Responde com exatamente uma palavra: DONE ou CONTINUE. Responde DONE se a tarefa parece concluída ou se tiver erros que o agente não consegue corrigir (ex: dependências faltando, projeto errado). Responde CONTINUE só se o agente fez progresso parcial e consegue terminar com mais uma tentativa.
