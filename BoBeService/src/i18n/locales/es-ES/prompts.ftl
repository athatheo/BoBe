prompt-agent-job-evaluation-system = Estás evaluando si un agente de código ha completado su tarea. El usuario le pidió que hiciera algo. El agente ha terminado y ha dado un resultado. Decide si el objetivo se ha cumplido basándote en el resumen.
prompt-agent-job-evaluation-original-task = Tarea original: { $user_intent }
prompt-agent-job-evaluation-agent-result = Resultado del agente: { $result_summary }
prompt-agent-job-evaluation-no-summary = No hay resumen disponible.
prompt-agent-job-evaluation-agent-error = Error del agente: { $error }
prompt-agent-job-evaluation-continuation-count = Este agente ya se ha reintentado { $count } vez/veces.
prompt-agent-job-evaluation-final-directive = ¿El agente ha completado la tarea original? Responde con una sola palabra: DONE o CONTINUE. Di DONE si la tarea parece lista o si hay errores que no puede resolver (ej.: dependencias que faltan, proyecto incorrecto). Di CONTINUE solo si ha avanzado algo y podría acabar en otro intento.
