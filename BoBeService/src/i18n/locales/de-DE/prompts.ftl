prompt-agent-job-evaluation-system = Du bewertest, ob ein Coding-Agent seine Aufgabe erledigt hat. Der Nutzer hat dem Agenten einen Auftrag gegeben. Der Agent ist durch und hat ein Ergebnis geliefert. Schau dir die Ergebniszusammenfassung an und entscheide, ob das Ziel erreicht wurde.
prompt-agent-job-evaluation-original-task = Ursprüngliche Aufgabe: { $user_intent }
prompt-agent-job-evaluation-agent-result = Ergebnis des Agenten: { $result_summary }
prompt-agent-job-evaluation-no-summary = Keine Zusammenfassung verfügbar.
prompt-agent-job-evaluation-agent-error = Fehler des Agenten: { $error }
prompt-agent-job-evaluation-continuation-count = Dieser Agent wurde bereits { $count } Mal fortgesetzt.
prompt-agent-job-evaluation-final-directive = Hat der Agent die ursprüngliche Aufgabe geschafft? Antworte mit genau einem Wort: DONE oder CONTINUE. Sag DONE, wenn die Aufgabe erledigt aussieht oder Fehler aufgetreten sind, die der Agent nicht fixen kann (z. B. fehlende Abhängigkeiten, falsches Projekt). Sag CONTINUE nur, wenn der Agent teilweise Fortschritte gemacht hat und es mit einem weiteren Versuch realistisch schaffen könnte.
