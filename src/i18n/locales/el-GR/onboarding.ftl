onboarding-local-tier-small-label = Μικρό (4B)
onboarding-local-tier-small-description = Γρήγορο, χαμηλή κατανάλωση πόρων. Ιδανικό για σύντομες κουβέντες.
onboarding-local-tier-medium-label = Μεσαίο (8B)
onboarding-local-tier-medium-description = Ισορροπία ανάμεσα σε ταχύτητα και ποιότητα.
onboarding-local-tier-large-label = Μεγάλο (14B)
onboarding-local-tier-large-description = Η καλύτερη ποιότητα — θέλει πιο δυνατό μηχάνημα.

onboarding-cloud-provider-openai-label = OpenAI
onboarding-cloud-provider-azure-openai-label = Azure OpenAI
onboarding-cloud-provider-openai-model-gpt-5-mini-label = GPT-5 Mini
onboarding-cloud-provider-openai-model-gpt-5-nano-label = GPT-5 Nano
onboarding-cloud-provider-openai-model-gpt-5-2-label = GPT-5.2

setup-error-create-data-directory = Δεν είναι δυνατή η δημιουργία του φακέλου δεδομένων: { $error }
setup-error-not-enough-disk-space = Δεν υπάρχει αρκετός χώρος στον δίσκο: απαιτούνται ~{ $needed_gb } GB, διαθέσιμα { $available_gb } GB
setup-error-unknown-provider = Άγνωστος πάροχος: { $provider }
setup-error-unknown-mode = Άγνωστη λειτουργία: { $mode }
setup-error-job-not-found = Η εργασία ρύθμισης '{ $job_id }' δεν βρέθηκε
setup-error-persist-failed = Η αποθήκευση ρύθμισης απέτυχε

setup-step-validate-data-directory-ready = Ο φάκελος δεδομένων είναι έτοιμος
setup-step-engine-ollama-at = Το Ollama βρίσκεται στο { $path }
setup-step-model-pulling = Pull του { $model }...
setup-step-model-ready = Το μοντέλο { $model } είναι έτοιμο
setup-step-vision-model-pull-failed-non-fatal = Αποτυχία pull μοντέλου vision (μη κρίσιμο): { $error }
setup-step-embedding-loading = Φόρτωση του μοντέλου embedding στη μνήμη...
setup-step-embedding-loaded = Το μοντέλο embedding φορτώθηκε
setup-step-embedding-warmup-failed-non-fatal = Η προθέρμανση απέτυχε (μη κρίσιμο): { $error }
setup-step-persist-saved = Η ρύθμιση αποθηκεύτηκε

setup-openai-error-api-key-required = Απαιτείται κλειδί API για το OpenAI
setup-openai-validation-api-key-valid = Το κλειδί API είναι έγκυρο
setup-openai-error-validation-http = Αποτυχία επαλήθευσης κλειδιού API: HTTP { $status }
setup-openai-error-invalid-api-key-format = Μη έγκυρη μορφή κλειδιού API OpenAI. Αφαίρεσε κενά ή αλλαγές γραμμής και δοκίμασε ξανά.
setup-openai-error-cannot-reach = Δεν είναι δυνατή η σύνδεση με το OpenAI: { $error }
setup-openai-embedding-testing = Έλεγχος τελικού σημείου embedding...
setup-openai-embedding-working = Το τελικό σημείο embedding λειτουργεί
setup-openai-embedding-failed = Η δοκιμή embedding απέτυχε: { $error }

setup-azure-error-api-key-required = Απαιτείται κλειδί API
setup-azure-error-endpoint-required = Απαιτείται τελικό σημείο
setup-azure-error-deployment-required = Απαιτείται όνομα deployment
setup-azure-validation-endpoint-validated = Το τελικό σημείο Azure επαληθεύτηκε
setup-azure-error-validation-http = Η επαλήθευση Azure απέτυχε: HTTP { $status }
setup-azure-error-invalid-value-format = Μη έγκυρη μορφή τιμής ρύθμισης Azure. Αφαίρεσε κενά ή αλλαγές γραμμής και δοκίμασε ξανά.
setup-azure-error-cannot-reach = Δεν είναι δυνατή η σύνδεση με το τελικό σημείο Azure: { $error }
setup-azure-embedding-testing = Έλεγχος embedding...
setup-azure-embedding-working = Το embedding λειτουργεί
setup-azure-embedding-failed = Ο έλεγχος embedding απέτυχε: { $error }
