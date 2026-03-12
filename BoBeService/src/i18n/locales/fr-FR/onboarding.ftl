onboarding-local-tier-small-label = Petit (4B)
onboarding-local-tier-small-description = Rapide et léger. Idéal pour des échanges courts.
onboarding-local-tier-medium-label = Moyen (8B)
onboarding-local-tier-medium-description = Bon équilibre entre performance et qualité.
onboarding-local-tier-large-label = Grand (14B)
onboarding-local-tier-large-description = Meilleure qualité, demande plus de ressources.

onboarding-cloud-provider-openai-label = OpenAI
onboarding-cloud-provider-azure-openai-label = Azure OpenAI
onboarding-cloud-provider-openai-model-gpt-5-mini-label = GPT-5 Mini
onboarding-cloud-provider-openai-model-gpt-5-nano-label = GPT-5 Nano
onboarding-cloud-provider-openai-model-gpt-5-2-label = GPT-5.2

setup-label-validate = Vérifier le système
setup-label-engine = Démarrer le moteur
setup-label-text-model = Télécharger le modèle texte
setup-label-embedding-model = Télécharger le modèle d'embedding
setup-label-embedding-warmup = Tester les embeddings
setup-label-vision-model = Télécharger le modèle de vision
setup-label-persist = Enregistrer la configuration

setup-step-validating = Vérification du système…
setup-step-engine-starting = Démarrage d'Ollama…
setup-step-persisting = Enregistrement de la configuration…

setup-error-create-data-directory = Impossible de créer le répertoire de données : { $error }
setup-error-not-enough-disk-space = Espace disque insuffisant : ~{ $needed_gb } Go requis, { $available_gb } Go disponibles
setup-error-unknown-provider = Fournisseur inconnu : { $provider }
setup-error-unknown-mode = Mode inconnu : { $mode }
setup-error-job-not-found = Tâche de configuration '{ $job_id }' introuvable
setup-error-persist-failed = Échec de l'enregistrement de la configuration

setup-step-validate-data-directory-ready = Répertoire de données prêt
setup-step-engine-ollama-at = Ollama à { $path }
setup-step-model-pulling = Téléchargement de { $model }
setup-step-model-ready = { $model } prêt
setup-step-vision-model-pull-failed-non-fatal = Échec du téléchargement du modèle de vision (non bloquant) : { $error }
setup-step-embedding-loading = Chargement du modèle d'embedding en mémoire...
setup-step-embedding-loaded = Modèle d'embedding chargé
setup-step-embedding-warmup-failed-non-fatal = Échec du préchauffage (non bloquant) : { $error }
setup-step-persist-saved = Configuration enregistrée

setup-openai-error-api-key-required = Une clé API est requise pour OpenAI
setup-openai-validation-api-key-valid = Clé API valide
setup-openai-error-validation-http = Échec de validation de la clé API : HTTP { $status }
setup-openai-error-invalid-api-key-format = Format de clé API OpenAI invalide. Retirez les espaces ou retours à la ligne, puis réessayez.
setup-openai-error-cannot-reach = Impossible d'atteindre OpenAI : { $error }
setup-openai-embedding-testing = Test du point de terminaison d'embedding...
setup-openai-embedding-working = Point de terminaison d'embedding opérationnel
setup-openai-embedding-failed = Échec du test d'embedding : { $error }

setup-azure-error-api-key-required = Clé API requise
setup-azure-error-endpoint-required = Point de terminaison requis
setup-azure-error-deployment-required = Déploiement requis
setup-azure-validation-endpoint-validated = Point de terminaison Azure validé
setup-azure-error-validation-http = Échec de la validation Azure : HTTP { $status }
setup-azure-error-invalid-value-format = Format de configuration Azure invalide. Retirez les espaces ou retours à la ligne, puis réessayez.
setup-azure-error-cannot-reach = Impossible de joindre le point de terminaison Azure : { $error }
setup-azure-embedding-testing = Test de l'embedding...
setup-azure-embedding-working = Embedding opérationnel
setup-azure-embedding-failed = Échec de l'embedding : { $error }
