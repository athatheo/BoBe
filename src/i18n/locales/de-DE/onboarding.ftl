onboarding-local-tier-small-label = Klein (4B)
onboarding-local-tier-small-description = Schnell, geringer Ressourcenverbrauch. Gut für kurze Interaktionen.
onboarding-local-tier-medium-label = Mittel (8B)
onboarding-local-tier-medium-description = Ausgewogene Leistung und Qualität.
onboarding-local-tier-large-label = Groß (14B)
onboarding-local-tier-large-description = Beste Qualität, benötigt mehr Ressourcen.

onboarding-cloud-provider-openai-label = OpenAI
onboarding-cloud-provider-azure-openai-label = Azure OpenAI
onboarding-cloud-provider-openai-model-gpt-5-mini-label = GPT-5 Mini
onboarding-cloud-provider-openai-model-gpt-5-nano-label = GPT-5 Nano
onboarding-cloud-provider-openai-model-gpt-5-2-label = GPT-5.2

setup-error-create-data-directory = Datenverzeichnis konnte nicht erstellt werden: { $error }
setup-error-not-enough-disk-space = Nicht genügend Speicherplatz: ~{ $needed_gb } GB erforderlich, { $available_gb } GB verfügbar
setup-error-unknown-provider = Unbekannter Anbieter: { $provider }
setup-error-unknown-mode = Unbekannter Modus: { $mode }
setup-error-job-not-found = Einrichtungsjob '{ $job_id }' nicht gefunden
setup-error-persist-failed = Konfiguration konnte nicht gespeichert werden

setup-step-validate-data-directory-ready = Datenverzeichnis bereit
setup-step-engine-ollama-at = Ollama unter { $path }
setup-step-model-pulling = { $model } wird heruntergeladen
setup-step-model-ready = { $model } bereit
setup-step-vision-model-pull-failed-non-fatal = Vision-Modell konnte nicht heruntergeladen werden (nicht kritisch): { $error }
setup-step-embedding-loading = Embedding-Modell wird in den Speicher geladen...
setup-step-embedding-loaded = Embedding-Modell geladen
setup-step-embedding-warmup-failed-non-fatal = Aufwärmphase fehlgeschlagen (nicht kritisch): { $error }
setup-step-persist-saved = Konfiguration gespeichert

setup-openai-error-api-key-required = Für OpenAI ist ein API-Schlüssel erforderlich
setup-openai-validation-api-key-valid = API-Schlüssel gültig
setup-openai-error-validation-http = Validierung des API-Schlüssels fehlgeschlagen: HTTP { $status }
setup-openai-error-invalid-api-key-format = Ungültiges Format für den OpenAI-API-Schlüssel. Entferne Leerzeichen/Zeilenumbrüche und versuche es erneut.
setup-openai-error-cannot-reach = OpenAI nicht erreichbar: { $error }
setup-openai-embedding-testing = Embedding-Endpoint wird getestet...
setup-openai-embedding-working = Embedding-Endpoint funktioniert
setup-openai-embedding-failed = Embedding-Test fehlgeschlagen: { $error }

setup-azure-error-api-key-required = API-Schlüssel erforderlich
setup-azure-error-endpoint-required = Endpunkt erforderlich
setup-azure-error-deployment-required = Deployment erforderlich
setup-azure-validation-endpoint-validated = Azure-Endpunkt validiert
setup-azure-error-validation-http = Azure-Validierung fehlgeschlagen: HTTP { $status }
setup-azure-error-invalid-value-format = Ungültiges Format für den Azure-Wert. Entferne Leerzeichen/Zeilenumbrüche und versuche es erneut.
setup-azure-error-cannot-reach = Azure-Endpunkt nicht erreichbar: { $error }
setup-azure-embedding-testing = Embedding wird getestet...
setup-azure-embedding-working = Embedding funktioniert
setup-azure-embedding-failed = Embedding fehlgeschlagen: { $error }
